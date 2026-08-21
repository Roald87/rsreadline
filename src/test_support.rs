//! Shared test-only helpers for unit tests across `src/*.rs`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

/// Writes `contents` to a uniquely-named file under the system temp dir so
/// parallel tests don't collide, and returns its path.
#[allow(clippy::unwrap_used)]
pub fn temp_history_file(prefix: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rsreadline_{prefix}_test_{}_{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, contents).unwrap();
    path
}
