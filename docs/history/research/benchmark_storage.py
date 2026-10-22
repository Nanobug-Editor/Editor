"""Compare history storage encodings without implementing Nanobug's history module.

The experiment uses native Zstd through ctypes and identical SQLite WAL/FULL
transactions for every encoding. It measures warm-cache storage operations,
not GPUI rendering, cold-device reads, or power-loss durability. Corpus and
databases stay in a newly created scratch directory supplied by the caller.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import sqlite3
import statistics
import struct
import sys
import time


# Fixed input generation and checkpoint spacing make repeated runs comparable.
SEED = 20261022
CHECKPOINT_INTERVAL = 16
MODES = ("raw", "rawcas", "zstd1", "zstd3", "chain16", "anchor16")


class Zstd:
    """Own reusable native contexts; encode/decode bytes with an optional raw dictionary."""

    def __init__(self, dll: Path):
        """Load the existing DLL at dll; no library or tool is installed."""
        self.lib = ctypes.CDLL(str(dll))
        declarations = {
            "ZSTD_versionString": (ctypes.c_char_p, []),
            "ZSTD_createCCtx": (ctypes.c_void_p, []),
            "ZSTD_createDCtx": (ctypes.c_void_p, []),
            "ZSTD_freeCCtx": (ctypes.c_size_t, [ctypes.c_void_p]),
            "ZSTD_freeDCtx": (ctypes.c_size_t, [ctypes.c_void_p]),
            "ZSTD_compressBound": (ctypes.c_size_t, [ctypes.c_size_t]),
            "ZSTD_isError": (ctypes.c_uint, [ctypes.c_size_t]),
            "ZSTD_getErrorName": (ctypes.c_char_p, [ctypes.c_size_t]),
            "ZSTD_compress_usingDict": (ctypes.c_size_t, [
                ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p,
                ctypes.c_size_t, ctypes.c_int,
            ]),
            "ZSTD_decompress_usingDict": (ctypes.c_size_t, [
                ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p,
                ctypes.c_size_t,
            ]),
        }
        for name, (result, arguments) in declarations.items():
            function = getattr(self.lib, name)
            function.restype, function.argtypes = result, arguments
        # Contexts belong to this sequential benchmark and are never shared between threads.
        self.compressor = self.lib.ZSTD_createCCtx()
        self.decompressor = self.lib.ZSTD_createDCtx()
        if not self.compressor or not self.decompressor:
            raise MemoryError("Zstd context allocation failed")

    def close(self):
        """Release both native contexts after the experiment."""
        self.lib.ZSTD_freeCCtx(self.compressor)
        self.lib.ZSTD_freeDCtx(self.decompressor)

    def check(self, result: int) -> int:
        """Return native output length, raising on any Zstd error code."""
        if self.lib.ZSTD_isError(result):
            raise RuntimeError(self.lib.ZSTD_getErrorName(result).decode())
        return result

    def encode(self, data: bytes, level: int, dictionary: bytes = b"") -> bytes:
        """Compress data at level, optionally referencing dictionary's complete bytes."""
        capacity = self.lib.ZSTD_compressBound(len(data))
        output = ctypes.create_string_buffer(capacity)
        written = self.check(self.lib.ZSTD_compress_usingDict(
            self.compressor, output, capacity, data, len(data),
            dictionary, len(dictionary), level,
        ))
        return output.raw[:written]

    def decode(self, data: bytes, size: int, dictionary: bytes = b"") -> bytes:
        """Restore exactly size bytes; dictionary must match the encoder's reference."""
        output = ctypes.create_string_buffer(size)
        written = self.check(self.lib.ZSTD_decompress_usingDict(
            self.decompressor, output, size, data, len(data),
            dictionary, len(dictionary),
        ))
        if written != size:
            raise AssertionError("decoded content length changed")
        return output.raw[:written]


def digest(data: bytes) -> bytes:
    """Return the SHA-256 identifier used for content and reconstruction verification."""
    return hashlib.sha256(data).digest()


def summary(values: list[float]) -> dict:
    """Summarize millisecond samples with median and nearest-rank p95."""
    ordered = sorted(values)
    return {"count": len(values), "p50_ms": statistics.median(values),
            "p95_ms": ordered[max(0, (len(ordered) * 95 + 99) // 100 - 1)]}


def create_corpus(repo: Path, scratch: Path) -> dict:
    """Write deterministic source-derived edit traces under scratch; return provenance."""
    rng = random.Random(SEED)
    sources = []
    pool = bytearray()
    for module in ("editor-core", "editor-ui", "editor-app"):
        for path in sorted((repo / "crates" / module / "src").rglob("*.rs")):
            contents = path.read_bytes()
            sources.append({"path": path.relative_to(repo).as_posix(),
                            "sha256": digest(contents).hex()})
            pool.extend(contents + b"\n")
    if len(pool) < 1024 * 1024:
        raise ValueError("source corpus is unexpectedly small")
    records = []
    with (scratch / "corpus.bin").open("wb") as output:
        for cohort, size in (("8KiB", 8192), ("64KiB", 65536), ("512KiB", 524288)):
            for pattern in range(4):
                start = rng.randrange(len(pool) - size)
                lines = bytes(pool[start:start + size]).decode("utf-8", "ignore").splitlines(True)
                history = []
                file_id = len(records) // 48
                for sequence in range(48):
                    # Independent traces cover sparse/distributed replacements, shifting
                    # insertions, and occasional broad rewrites. Repeats model no-op/undo saves.
                    if sequence and sequence % 9 == 0:
                        lines = history[-3].decode("utf-8").splitlines(True)
                    elif sequence and sequence % 7 != 0:
                        if pattern == 2:
                            lines.insert(0, f"// inserted version {sequence}: {rng.getrandbits(128):032x}\n")
                        else:
                            changes = 1 if pattern == 0 else 8
                            if pattern == 3 and sequence % 12 == 0:
                                changes = max(1, len(lines) // 2)
                            for position in rng.sample(range(len(lines)), min(changes, len(lines))):
                                lines[position] = f"// changed {sequence}: {rng.getrandbits(192):048x}\n"
                    contents = "".join(lines).encode("utf-8")
                    history.append(contents)
                    records.append({"file": file_id, "sequence": sequence,
                                    "cohort": cohort, "offset": output.tell(),
                                    "size": len(contents), "sha256": digest(contents).hex()})
                    output.write(contents)
    manifest = {"seed": SEED, "source_files": sources, "records": records,
                "logical_bytes": sum(record["size"] for record in records),
                "corpus_sha256": digest((scratch / "corpus.bin").read_bytes()).hex()}
    (scratch / "corpus.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest


def connect(path: Path) -> sqlite3.Connection:
    """Open path with the same page/cache/durability settings for every candidate."""
    db = sqlite3.connect(path)
    db.execute("PRAGMA page_size=4096")
    db.execute("PRAGMA journal_mode=WAL")
    db.execute("PRAGMA synchronous=FULL")
    db.execute("PRAGMA cache_size=-8192")
    db.execute("PRAGMA mmap_size=0")
    db.execute("PRAGMA wal_autocheckpoint=1000")
    return db


def read_object(db: sqlite3.Connection, codec: Zstd, key: bytes, memo: dict) -> bytes:
    """Return key's bytes from db using codec; memo is local to this read's reference graph."""
    if key in memo:
        return memo[key]
    encoding, reference, size, payload = db.execute(
        "SELECT encoding, reference, raw_size, payload FROM objects WHERE id=?", (key,)
    ).fetchone()
    if encoding == "raw":
        result = payload
    else:
        dictionary = read_object(db, codec, reference, memo) if reference else b""
        result = codec.decode(payload, size, dictionary)
    memo[key] = result
    return result


def create_object(codec: Zstd, mode: str, contents: bytes, record: dict,
                  state: dict, known: dict, content_objects: dict) -> tuple:
    """Choose an object for contents using codec and the selected mode.

    record supplies file/sequence; state retains this file's previous/anchor
    content. known maps persisted keys to depth; content_objects maps content
    digests to immutable encodings. Return key, encoding, reference, depth and
    payload; a reused object needs no new payload.
    """
    checksum = digest(contents)
    reference, dictionary, depth = None, b"", 0
    level = 3 if mode == "zstd3" else 1
    checkpoint = mode in ("chain16", "anchor16") and record["sequence"] % CHECKPOINT_INTERVAL == 0
    if mode != "raw" and not checkpoint and checksum in content_objects:
        # Reuse an immutable object's original reference graph; changing its dictionary
        # would invalidate any existing entry that points to this object.
        key, encoding, reference, depth = content_objects[checksum]
        return key, encoding, reference, depth, None
    if mode in ("chain16", "anchor16"):
        if not checkpoint and state:
            if mode == "chain16" and state["depth"] < CHECKPOINT_INTERVAL - 1:
                reference, dictionary, depth = state["key"], state["contents"], state["depth"] + 1
            elif mode == "anchor16":
                reference, dictionary, depth = state["anchor_key"], state["anchor_contents"], 1
    if mode in ("raw", "rawcas"):
        identity = struct.pack("<II", record["file"], record["sequence"]) if mode == "raw" else b""
        key = digest(b"raw" + identity + checksum)
        encoding, payload = "raw", contents
    else:
        encoding = "dict" if reference else "zstd"
        key = digest(encoding.encode() + (reference or b"") + checksum)
        payload = None if key in known else codec.encode(contents, level, dictionary)
    content_objects[checksum] = (key, encoding, reference, depth)
    return key, encoding, reference, depth, payload


def verify_records(db: sqlite3.Connection, codec: Zstd, records: list[dict], keys: dict):
    """Decode records from db through codec and keys; require matching complete content hashes."""
    for record in records:
        result = read_object(db, codec, keys[record["file"], record["sequence"]], {})
        if digest(result).hex() != record["sha256"]:
            raise AssertionError(f"content changed: {record['file']}:{record['sequence']}")


def garbage_collect(db: sqlite3.Connection) -> dict:
    """Delete the oldest half of entries, retain all reachable bases, and time compaction."""
    started = time.perf_counter_ns()
    db.execute("DELETE FROM entries WHERE sequence < 24")
    references = dict(db.execute("SELECT id, reference FROM objects"))
    reachable, pending = set(), [row[0] for row in db.execute("SELECT object_id FROM entries")]
    while pending:
        key = pending.pop()
        if key not in reachable:
            reachable.add(key)
            if references[key] is not None:
                pending.append(references[key])
    db.executemany("DELETE FROM objects WHERE id=?", [(key,) for key in references if key not in reachable])
    db.commit()
    delete_ms = (time.perf_counter_ns() - started) / 1e6
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    started = time.perf_counter_ns()
    db.execute("VACUUM")
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    vacuum_ms = (time.perf_counter_ns() - started) / 1e6
    return {"delete_ms": delete_ms, "vacuum_ms": vacuum_ms,
            "remaining_objects": len(reachable), "deleted_objects": len(references) - len(reachable)}


def index_probe(scratch: Path) -> dict:
    """Measure newest-50 metadata queries with one million entries, without reading contents."""
    db = connect(scratch / "index-probe.sqlite")
    db.execute("CREATE TABLE entries(file INTEGER, sequence INTEGER, object_id BLOB, timestamp INTEGER, "
               "PRIMARY KEY(file, sequence)) WITHOUT ROWID")
    db.executemany("INSERT INTO entries VALUES(?,?,?,?)",
                   ((file_id, sequence, b"x" * 32, sequence)
                    for file_id in range(1000) for sequence in range(1000)))
    db.commit()
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    samples = []
    for repeat in range(3):
        order = list(range(1000))
        random.Random(SEED + repeat).shuffle(order)
        for file_id in order:
            started = time.perf_counter_ns()
            rows = db.execute("SELECT sequence,object_id,timestamp FROM entries WHERE file=? "
                              "ORDER BY sequence DESC LIMIT 50", (file_id,)).fetchall()
            samples.append((time.perf_counter_ns() - started) / 1e6)
            if len(rows) != 50 or rows[0][0] != 999:
                raise AssertionError("metadata query lost or reordered entries")
    db.close()
    return {"rows": 1000000, "files": 1000, "list": summary(samples), "samples_ms": samples}


def benchmark(codec: Zstd, mode: str, repeat: int, scratch: Path, manifest: dict) -> dict:
    """Measure mode with native codec in a fresh repeat database under scratch.

    manifest describes the immutable corpus. Return raw millisecond latency
    samples, byte counts and validation-backed garbage collection measurements.
    """
    records = manifest["records"]
    corpus = (scratch / "corpus.bin").read_bytes()
    path = scratch / f"{mode}-{repeat}.sqlite"
    db = connect(path)
    db.executescript("""
        CREATE TABLE objects(id BLOB PRIMARY KEY, encoding TEXT NOT NULL,
            reference BLOB, raw_size INTEGER NOT NULL, payload BLOB NOT NULL);
        CREATE TABLE entries(file INTEGER NOT NULL, sequence INTEGER NOT NULL,
            object_id BLOB NOT NULL, timestamp INTEGER NOT NULL,
            PRIMARY KEY(file, sequence)) WITHOUT ROWID;
    """)
    states, known, content_objects, keys, write_samples = {}, {}, {}, {}, []
    cpu_started = time.process_time_ns()
    for record in records:
        contents = corpus[record["offset"]:record["offset"] + record["size"]]
        state = states.setdefault(record["file"], {})
        started = time.perf_counter_ns()
        key, encoding, reference, depth, payload = create_object(
            codec, mode, contents, record, state, known, content_objects)
        if key not in known:
            db.execute("INSERT INTO objects VALUES(?,?,?,?,?)", (key, encoding, reference, len(contents), payload))
            known[key] = depth
        db.execute("INSERT INTO entries VALUES(?,?,?,?)", (record["file"], record["sequence"], key, record["sequence"]))
        db.commit()
        write_samples.append({"cohort": record["cohort"], "ms": (time.perf_counter_ns() - started) / 1e6})
        keys[record["file"], record["sequence"]] = key
        state.update(key=key, contents=contents, depth=known[key])
        if mode == "anchor16" and record["sequence"] % CHECKPOINT_INTERVAL == 0:
            state.update(anchor_key=key, anchor_contents=contents)
    write_cpu_ms = (time.process_time_ns() - cpu_started) / 1e6
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    db.close()
    stored_bytes = path.stat().st_size
    # Reopen and verify all versions before the measured warm-cache queries.
    db = connect(path)
    verify_records(db, codec, records, keys)
    rng = random.Random(SEED + repeat)
    order = list(records)
    rng.shuffle(order)
    read_samples, list_samples = [], []
    for record in order:
        started = time.perf_counter_ns()
        result = read_object(db, codec, keys[record["file"], record["sequence"]], {})
        read_samples.append({"cohort": record["cohort"], "ms": (time.perf_counter_ns() - started) / 1e6})
        if len(result) != record["size"]:
            raise AssertionError("read size changed")
        started = time.perf_counter_ns()
        rows = db.execute("SELECT sequence, object_id, timestamp FROM entries WHERE file=? ORDER BY sequence DESC LIMIT 50",
                          (record["file"],)).fetchall()
        list_samples.append((time.perf_counter_ns() - started) / 1e6)
        if len(rows) != 48:
            raise AssertionError("history index lost entries")
    started = time.perf_counter_ns()
    gc_result = garbage_collect(db)
    gc_result["total_ms"] = (time.perf_counter_ns() - started) / 1e6
    verify_records(db, codec, [record for record in records if record["sequence"] >= 24], keys)
    db.close()
    return {"mode": mode, "repeat": repeat, "database_bytes": stored_bytes,
            "after_gc_bytes": path.stat().st_size, "write_cpu_ms": write_cpu_ms,
            "object_count": len(known), "gc": gc_result,
            "write": summary([sample["ms"] for sample in write_samples]),
            "read": summary([sample["ms"] for sample in read_samples]),
            "list": summary(list_samples), "write_samples": write_samples,
            "read_samples": read_samples, "list_samples_ms": list_samples}


def main():
    """Create an isolated experiment from CLI paths and write reproducible JSON evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    parser.add_argument("--dll", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=3)
    args = parser.parse_args()
    # Requiring a new directory prevents accidental replacement of another experiment.
    args.scratch.mkdir(parents=True, exist_ok=False)
    manifest = create_corpus(args.repo, args.scratch)
    codec = Zstd(args.dll)
    evidence = {"environment": {"python": sys.version, "platform": platform.platform(),
                "cpu": os.environ.get("PROCESSOR_IDENTIFIER"), "cores": os.cpu_count(),
                "sqlite": sqlite3.sqlite_version,
                "zstd": codec.lib.ZSTD_versionString().decode(), "dll": str(args.dll)},
                "corpus": {key: value for key, value in manifest.items() if key != "records"},
                "versions": len(manifest["records"]), "scratch": str(args.scratch), "runs": []}
    try:
        evidence["index_probe"] = index_probe(args.scratch)
        for repeat in range(args.repeats):
            modes = list(MODES)
            random.Random(SEED + repeat).shuffle(modes)
            for mode in modes:
                result = benchmark(codec, mode, repeat, args.scratch, manifest)
                evidence["runs"].append(result)
                args.report.parent.mkdir(parents=True, exist_ok=True)
                args.report.write_text(json.dumps(evidence, indent=2), encoding="utf-8")
                print(json.dumps({key: result[key] for key in
                                 ("mode", "repeat", "database_bytes", "write", "read", "list", "gc")}), flush=True)
    finally:
        codec.close()


if __name__ == "__main__":
    main()
