//! Reproducible product-store measurements; invoke with fresh scratch, immutable corpus and report paths.

use editor_history::{Capture, CaptureOutcome, ClearScope, History, Policy, Query, Reason, Stats};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    error::Error,
    path::{Path, PathBuf},
    time::Instant,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Separate new-object commits, shared-object commits and unchanged saves without subtracting timings.
#[derive(Default)]
struct CaptureClasses {
    known: HashSet<Vec<u8>>,
    fresh: Vec<f64>,
    shared: Vec<f64>,
    unchanged: Vec<f64>,
}

impl CaptureClasses {
    /// Record the actual public result; independent digest work is outside the durable-call timing.
    fn record(&mut self, digest: Vec<u8>, outcome: CaptureOutcome, took: f64) {
        let known = !self.known.insert(digest);
        match outcome {
            CaptureOutcome::Recorded(_) if known => self.shared.push(took),
            CaptureOutcome::Recorded(_) => self.fresh.push(took),
            CaptureOutcome::Unchanged(_) => self.unchanged.push(took),
            CaptureOutcome::Skipped(_) => unreachable!("benchmark text must be eligible"),
        }
    }

    /// Percentiles include verification and SQLite FULL commits in the applicable class.
    fn report(self) -> Value {
        json!({"new_object":summary(self.fresh), "shared_object_new_version":summary(self.shared),
            "unchanged":summary(self.unchanged)})
    }
}

/// Percentiles describe complete synchronous durable operations; no background acknowledgement shortcut.
fn summary(mut times: Vec<f64>) -> Value {
    times.sort_by(f64::total_cmp);
    if times.is_empty() {
        return json!({"samples": 0});
    }
    let percentile = |fraction: f64| times[((times.len() - 1) as f64 * fraction).ceil() as usize];
    json!({"samples": times.len(), "p50_ms": percentile(0.5), "p95_ms": percentile(0.95), "max_ms": times[times.len()-1]})
}

/// Record actual main/WAL allocation separately from compressed object and metadata counters.
fn usage(stats: Stats) -> Value {
    json!({"versions": stats.versions, "objects": stats.objects, "raw_distinct_bytes": stats.raw_bytes,
        "compressed_object_bytes": stats.compressed_bytes, "database_bytes": stats.disk_bytes,
        "wal_bytes": stats.wal_bytes, "legacy_bytes": stats.legacy_bytes, "pending_gc_bytes": stats.pending_gc_bytes})
}

fn elapsed(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.
}

/// Logical-only project paths do not depend on TEMP, which the harness redirects to observe SQLite spill.
fn document(name: impl AsRef<Path>) -> PathBuf {
    std::env::current_dir()
        .expect("benchmark working directory")
        .join("NanobugHistoryBenchmark")
        .join(name)
}

/// Require new output state so a previous benchmark can never silently alter the measured trace.
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err(
            "usage: benchmark <fresh-scratch> <immutable-corpus-directory> <report.json>".into(),
        );
    }
    std::fs::create_dir(&args[0])?;
    let manifest: Value = serde_json::from_slice(&std::fs::read(args[1].join("corpus.json"))?)?;
    let corpus = std::fs::read(args[1].join("corpus.bin"))?;
    let digest = format!("{:x}", Sha256::digest(&corpus));
    if Some(digest.as_str()) != manifest["corpus_sha256"].as_str() {
        return Err("immutable corpus digest mismatch".into());
    }
    let mut report = json!({"corpus_sha256": digest, "profile": "release", "sqlite_sync": "FULL/WAL",
        "zstd_level": 1, "repeats": 3, "cache": "OS cache not flushed; first decode after reopen and warm rereads reported separately",
        "phase_samples": "hash/codec sampled independently; capture totals include their actual internal work and SQLite FULL commit",
        "scratch": args[0], "mixed": []});
    for repeat in 0..3 {
        report["mixed"]
            .as_array_mut()
            .unwrap()
            .push(mixed(&args[0], repeat, &manifest, &corpus)?);
        std::fs::write(&args[2], serde_json::to_vec_pretty(&report)?)?;
        eprintln!("mixed repeat {repeat} complete");
    }
    // Large-file peak measurements should not retain the unrelated immutable mixed corpus in RAM.
    drop(corpus);
    report["large"] = large(&args[0])?;
    report["long_history"] = long_history(&args[0])?;
    report["near_budget"] = near_budget(&args[0])?;
    std::fs::write(&args[2], serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

/// Reuse every immutable source record, including no-op and content-return events, without resampling sources.
fn mixed(scratch: &Path, repeat: usize, manifest: &Value, corpus: &[u8]) -> Result<Value> {
    let root = scratch.join(format!("mixed-{repeat}"));
    let history = History::open(&root, Policy::default())?;
    let mut writes = Vec::new();
    let mut hashes = Vec::new();
    let mut codecs = Vec::new();
    let mut versions = Vec::new();
    let mut classes = CaptureClasses::default();
    let mut raw = 0_u64;
    for record in manifest["records"].as_array().ok_or("missing records")? {
        let offset = record["offset"].as_u64().ok_or("offset")? as usize;
        let size = record["size"].as_u64().ok_or("size")? as usize;
        let bytes = &corpus[offset..offset + size];
        raw += size as u64;
        let started = Instant::now();
        let digest = Sha256::digest(bytes).to_vec();
        std::hint::black_box(&digest);
        hashes.push(elapsed(started));
        let started = Instant::now();
        std::hint::black_box(zstd::stream::encode_all(bytes, 1)?);
        codecs.push(elapsed(started));
        let capture = Capture {
            file: None,
            path: document(format!("file-{}.txt", record["file"])),
            workspace: Some(document("workspace")),
            contents: bytes.to_vec(),
            reason: Reason::Saved,
            at_ms: 2_000_000_000_000 + versions.len() as i64,
        };
        let started = Instant::now();
        let result = history.capture(capture)?;
        let took = elapsed(started);
        writes.push(took);
        classes.record(digest.clone(), result, took);
        versions.push((result.version().ok_or("unexpected skip")?, digest));
    }
    let before = usage(history.stats()?);
    history.maintain(2_000_000_000_001)?;
    let compacted = usage(history.stats()?);
    drop(history);
    let history = History::open(root, Policy::default())?;
    let mut first = Vec::new();
    let mut warm = Vec::new();
    for pass in 0..2 {
        for index in (0..versions.len()).map(|index| index * 137 % versions.len()) {
            let (version, digest) = &versions[index];
            let started = Instant::now();
            let bytes = history.read(*version)?;
            let took = elapsed(started);
            if Sha256::digest(&bytes).as_slice() != digest {
                return Err("read digest mismatch".into());
            }
            if pass == 0 {
                first.push(took);
            } else {
                warm.push(took);
            }
        }
    }
    Ok(
        json!({"repeat": repeat, "original_capture_bytes": raw, "capture": summary(writes), "sha256": summary(hashes),
        "zstd1": summary(codecs), "first_decode_after_reopen": summary(first), "warm_decode": summary(warm),
        "before_compaction": before, "after_compaction": compacted, "success": versions.len(), "capture_classes":classes.report()}),
    )
}

/// Deterministic ASCII text includes low-compressibility rewrites as well as sparse and front edits.
fn text(size: usize, seed: u64, multiline: bool) -> Vec<u8> {
    let mut state = seed | 1;
    (0..size)
        .map(|index| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            if multiline && index % 96 == 95 {
                b'\n'
            } else {
                b' ' + (state % 95) as u8
            }
        })
        .collect()
}

fn large(scratch: &Path) -> Result<Value> {
    let mut results = Vec::new();
    for size in [
        8 * 1024,
        64 * 1024,
        512 * 1024,
        1024 * 1024,
        8 * 1024 * 1024,
        32 * 1024 * 1024,
    ] {
        let history = History::open(
            scratch.join(format!("large-{size}")),
            Policy {
                max_file_bytes: 32 * 1024 * 1024,
                ..Default::default()
            },
        )?;
        let mut capture_times = Vec::new();
        let mut read_times = Vec::new();
        let mut hashes = Vec::new();
        let mut codecs = Vec::new();
        let mut patterns = Vec::new();
        let mut classes = CaptureClasses::default();
        let mut version_count = 0;
        for multiline in [false, true] {
            let original = text(size, 20261022, multiline);
            for sequence in 0..6 {
                let mut bytes = if sequence == 3 {
                    text(size, 42, multiline)
                } else {
                    original.clone()
                };
                if sequence == 1 {
                    bytes[size / 2] = b'!';
                }
                if sequence == 2 {
                    bytes.rotate_right(16);
                    bytes[..16].copy_from_slice(b"// front insert\n");
                }
                let started = Instant::now();
                let digest = Sha256::digest(&bytes).to_vec();
                hashes.push(elapsed(started));
                let started = Instant::now();
                std::hint::black_box(zstd::stream::encode_all(bytes.as_slice(), 1)?);
                codecs.push(elapsed(started));
                let input = Capture {
                    file: None,
                    path: document(format!("large-{size}-{multiline}.txt")),
                    workspace: None,
                    contents: bytes,
                    reason: Reason::Saved,
                    at_ms: 2_000_000_000_000 + sequence as i64,
                };
                let started = Instant::now();
                let outcome = history.capture(input)?;
                let took = elapsed(started);
                let version = outcome.version().ok_or("large file skipped")?;
                capture_times.push(took);
                classes.record(digest.clone(), outcome, took);
                version_count += 1;
                let started = Instant::now();
                let decoded = history.read(version)?;
                let read_ms = elapsed(started);
                if Sha256::digest(&decoded).as_slice() != digest {
                    return Err("large-file read digest mismatch".into());
                }
                read_times.push(read_ms);
                let pattern = [
                    "original",
                    "sparse",
                    "front",
                    "rewrite",
                    "return",
                    "duplicate",
                ][sequence];
                patterns.push(json!({"multiline":multiline, "pattern":pattern,
                    "capture_ms":took,"read_ms":read_ms}));
            }
        }
        let before = usage(history.stats()?);
        history.maintain(2_000_000_000_001)?;
        results.push(json!({"size": size, "operations": version_count, "patterns": ["single line", "multiline", "sparse", "front", "rewrite", "return", "duplicate"],
            "capture": summary(capture_times), "read": summary(read_times), "sha256":summary(hashes),"zstd1":summary(codecs),
            "capture_classes":classes.report(),"pattern_samples":patterns,"before": before, "after": usage(history.stats()?)}));
        eprintln!("large {size} complete");
    }
    Ok(json!(results))
}

/// Read old/new direct objects and query both ends of a long stable cursor sequence.
fn long_history(scratch: &Path) -> Result<Value> {
    let history = History::open(scratch.join("long"), Policy::default())?;
    let path = document("long.txt");
    let mut versions = Vec::new();
    for sequence in 0..2048 {
        let bytes = format!("{sequence:08}{}", "source line\n".repeat(700)).into_bytes();
        versions.push(
            history
                .capture(Capture {
                    file: None,
                    path: path.clone(),
                    workspace: Some(document("workspace")),
                    contents: bytes,
                    reason: Reason::Saved,
                    at_ms: 2_000_000_000_000 + sequence,
                })
                .map(|outcome| outcome.version().unwrap())?,
        );
    }
    let mut reads = [Vec::new(), Vec::new()];
    let mut pages = Vec::new();
    let mut filters = Vec::new();
    for _ in 0..100 {
        for (side, index) in [0, 2047].into_iter().enumerate() {
            let started = Instant::now();
            std::hint::black_box(history.read(versions[index])?);
            reads[side].push(elapsed(started));
        }
        let started = Instant::now();
        let first = history.list(&Query::default())?;
        history.list(&Query {
            before: first.last().map(|row| row.cursor()),
            ..Default::default()
        })?;
        pages.push(elapsed(started));
        let started = Instant::now();
        history.list(&Query {
            path: "long".into(),
            ..Default::default()
        })?;
        filters.push(elapsed(started));
    }
    Ok(
        json!({"changes": 2048, "oldest": summary(reads[0].clone()), "newest": summary(reads[1].clone()),
        "first_and_second_50_query": summary(pages), "path_filter": summary(filters), "usage": usage(history.stats()?)}),
    )
}

/// Capacity eviction, a live preview lease and final physical compaction share the actual serial store.
fn near_budget(scratch: &Path) -> Result<Value> {
    let policy = Policy {
        max_total_bytes: 512 * 1024,
        ..Default::default()
    };
    let history = History::open(scratch.join("budget"), policy.clone())?;
    let mut times = Vec::new();
    let mut peak_wal = 0;
    let mut peak_database = 0;
    for sequence in 0..96 {
        let started = Instant::now();
        history.capture(Capture {
            file: None,
            path: document("budget.txt"),
            workspace: None,
            contents: text(64 * 1024, sequence + 1, true),
            reason: Reason::Saved,
            at_ms: 2_000_000_000_000 + sequence as i64,
        })?;
        times.push(elapsed(started));
        let stats = history.stats()?;
        peak_wal = peak_wal.max(stats.wal_bytes);
        peak_database = peak_database.max(stats.disk_bytes);
    }
    let newest = history.list(&Query::default())?[0].id;
    let lease = history.pin(newest)?;
    let started = Instant::now();
    history.clear(&ClearScope::All)?;
    let clear_ms = elapsed(started);
    history.read(lease.version())?;
    let leased = usage(history.stats()?);
    drop(lease);
    let started = Instant::now();
    history.maintain(2_000_000_000_001)?;
    let compact_ms = elapsed(started);
    let after = history.stats()?;
    if after.disk_bytes + after.legacy_bytes + after.pending_gc_bytes > policy.max_total_bytes {
        return Err("physical capacity exceeds budget".into());
    }
    Ok(
        json!({"budget": policy.max_total_bytes, "capture_under_eviction": summary(times), "peak_wal_bytes": peak_wal,
        "peak_database_bytes": peak_database, "clear_ms": clear_ms, "leased_after_clear": leased,
        "maintenance_ms": compact_ms, "after": usage(after)}),
    )
}
