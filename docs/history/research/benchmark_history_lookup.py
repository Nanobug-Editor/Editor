"""Stress direct version lookup and old-reference metadata replay at longer history depths.

Two alternating complete 8 KiB objects isolate lookup cost from large-file I/O.
The fixture retains 2048 captures; comparisons use the same durable SQLite
settings as benchmark_history_models.py and verify every measured read's hash.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import random
import sys
import time

# Imports must not create generated artifacts beside the reproducible sources.
sys.dont_write_bytecode = True
import benchmark_history_models as models


def run_lookup(scratch: Path, contents: bytes, zstd: models.common.Zstd, lz4: models.Lz4) -> list[dict]:
    """Build snapshot/log fixtures under scratch and measure ages with existing native codecs."""
    bodies = [contents, contents + b"\n// alternate state\n"]
    expected = [models.common.digest(body) for body in bodies]
    output = []
    for mode in ("snapshot_zstd", "oldref_zstd"):
        db = models.common.connect(scratch / f"{mode}.sqlite")
        model = models.HistoryModel(db, mode, zstd, lz4)
        for mark in range(2048):
            event = {"file": 0, "mark": mark, "id": mark + 1, "kind": "write",
                     "path": "src/lookup.rs", "present": True}
            model.capture(event, bodies[mark % 2])
        db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        db.close()
        model.db = models.common.connect(scratch / f"{mode}.sqlite")
        strategies = ("replay", "jump") if model.log else ("direct",)
        for strategy in strategies:
            samples = {age: [] for age in (0, 16, 256, 1024, 2047)}
            order = [age for age in samples for _ in range(100)]
            random.Random(models.common.SEED).shuffle(order)
            for age in order:
                mark = 2047 - age
                started = time.perf_counter_ns()
                path, alive, body = model.read(0, mark, strategy)
                samples[age].append((time.perf_counter_ns() - started) / 1e6)
                if path != "src/lookup.rs" or not alive or models.common.digest(body) != expected[mark % 2]:
                    raise AssertionError("deep-history state changed")
            output.append({"mode": mode, "strategy": strategy,
                           "ages": {str(age): {"summary": models.common.summary(values), "samples_ms": values}
                                    for age, values in samples.items()}})
        model.db.close()
    return output


def main():
    """Read the immutable source corpus and write the isolated lookup experiment's JSON results."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    parser.add_argument("--zstd-dll", type=Path, required=True)
    parser.add_argument("--lz4-dll", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    args.scratch.mkdir(parents=True, exist_ok=False)
    manifest = json.loads((args.corpus / "corpus.json").read_text(encoding="utf-8"))
    corpus = (args.corpus / "corpus.bin").read_bytes()
    if models.common.digest(corpus).hex() != manifest["corpus_sha256"]:
        raise AssertionError("immutable corpus changed")
    record = manifest["records"][0]
    zstd, lz4 = models.common.Zstd(args.zstd_dll), models.Lz4(args.lz4_dll)
    try:
        results = run_lookup(args.scratch, corpus[record["offset"]:record["offset"] + record["size"]], zstd, lz4)
        args.report.write_text(json.dumps({"captures_per_model": 2048, "scratch": str(args.scratch),
                                          "corpus_sha256": manifest["corpus_sha256"], "results": results}, indent=2),
                               encoding="utf-8")
        print(json.dumps([{key: value for key, value in result.items() if key != "ages"} |
                          {"p95_ms_by_age": {age: value["summary"]["p95_ms"] for age, value in result["ages"].items()}}
                          for result in results]), flush=True)
    finally:
        zstd.close()


if __name__ == "__main__":
    main()
