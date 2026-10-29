//! Bounded immutable comparison results shared by decorations and native row correspondence.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    sync::{Arc, LazyLock, Mutex},
};

/// No text or document authority is retained; exact source digests key immutable byte ranges.
struct Entry {
    key: [[u8; 32]; 2],
    hunks: Arc<[Hunk]>,
}
/// LRU storage is bounded by both result count and bytes, including adversarial many-hunk inputs.
static RESULTS: LazyLock<Mutex<VecDeque<Entry>>> = LazyLock::new(|| Mutex::new(VecDeque::new()));

/// Check source bounds before hashing; compute outside the shared lock and return owned ranges.
pub(super) fn compare(left: &str, right: &str) -> Result<Vec<Hunk>, ComparisonError> {
    if left.len() > MAX_SOURCE_BYTES || right.len() > MAX_SOURCE_BYTES {
        return Err(ComparisonError::SourceBytes);
    }
    let key = [
        Sha256::digest(left.as_bytes()).into(),
        Sha256::digest(right.as_bytes()).into(),
    ];
    {
        let mut entries = RESULTS.lock().unwrap();
        if let Some(index) = entries.iter().position(|entry| entry.key == key) {
            let entry = entries.remove(index).unwrap();
            let hunks = entry.hunks.clone();
            entries.push_back(entry);
            return Ok(hunks.to_vec());
        }
    }
    let hunks = compare_uncached(left, right)?;
    let shared: Arc<[Hunk]> = hunks.clone().into();
    let mut entries = RESULTS.lock().unwrap();
    entries.retain(|entry| entry.key != key);
    entries.push_back(Entry { key, hunks: shared });
    while entries.len() > 16
        || entries
            .iter()
            .map(|entry| std::mem::size_of_val(entry.hunks.as_ref()))
            .sum::<usize>()
            > 4 * 1024 * 1024
    {
        entries.pop_front();
    }
    Ok(hunks)
}
