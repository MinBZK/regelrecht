//! Helpers shared by unit tests. The crate has no `tempfile` dependency, and a
//! scratch directory is all the tests need.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A scratch directory under the system temp dir, removed on drop.
pub struct Dir(PathBuf);

impl Dir {
    #[allow(clippy::expect_used)]
    pub fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "code-guide-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("creating scratch dir");
        Dir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Writes `contents` to `rel` under the directory, creating parents.
    #[allow(clippy::expect_used)]
    pub fn write(&self, rel: &str, contents: &str) {
        let file = self.0.join(rel);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("creating parent dirs");
        }
        std::fs::write(file, contents).expect("writing file");
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
