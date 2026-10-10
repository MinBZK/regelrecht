//! Where a law's stand-off notes sidecar lives in a source repository.
//!
//! RFC-018 §2 puts it at `annotations/{law_id}/annotations.yaml` at the
//! **repository root**, next to `regulation/nl/`, not under the source's
//! regulation subtree. A source configured with a subpath (e.g.
//! `regulation/nl`) therefore reaches its sidecars through the
//! repository-root methods on [`RepoBackend`].
//!
//! Earlier editor versions wrote the sidecar under the subpath instead
//! (`regulation/nl/annotations/...`). Reads fall back to that legacy
//! location when the repository-root file does not exist, so notes saved
//! there stay visible; the next save writes the merged result to the
//! repository root and leaves the legacy file where it is.

use std::path::{Component, Path, PathBuf};

use crate::backend::RepoBackend;
use crate::error::{CorpusError, Result};

/// The sidecar's path, relative to the repository root.
///
/// `law_id` comes from the request URL, so it must be one plain path
/// segment: no separators, no `.`/`..`, nothing empty. The backends
/// reject `..` and absolute paths on their own; this check keeps the
/// sidecar inside its own `annotations/{law_id}/` directory as well.
pub fn sidecar_path(law_id: &str) -> Result<PathBuf> {
    let mut components = Path::new(law_id).components();
    let single_normal =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if !single_normal || law_id.contains(['/', '\\', '\0']) {
        return Err(CorpusError::Config(format!(
            "law id is not a single path segment: {law_id:?}"
        )));
    }
    Ok(PathBuf::from("annotations")
        .join(law_id)
        .join("annotations.yaml"))
}

/// Whether [`read_sidecar`] consults the legacy location under the source
/// subpath when the repository-root file does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyFallback {
    /// Consult it: traject repositories, where earlier editor versions
    /// saved notes under the subpath.
    Read,
    /// Do not: sources the editor never wrote to (the central corpus read
    /// without a traject), where the fallback would only cost a second
    /// request per law.
    Skip,
}

/// Read a law's sidecar: the repository-root file, or, when that does not
/// exist, the source has a subpath and `fallback` allows it, the legacy
/// file under the subpath. Without a subpath both locations are the same
/// file, read once.
pub async fn read_sidecar(
    backend: &dyn RepoBackend,
    law_id: &str,
    token_override: Option<&str>,
    fallback: LegacyFallback,
) -> Result<Option<String>> {
    let path = sidecar_path(law_id)?;
    if let Some(text) = backend
        .read_repo_file_with_token(&path, token_override)
        .await?
    {
        return Ok(Some(text));
    }
    if fallback == LegacyFallback::Skip || backend.repo_subpath().is_none() {
        return Ok(None);
    }
    backend.read_file_with_token(&path, token_override).await
}

/// Stage a write of a law's sidecar at the repository root. Committed by
/// the backend's next `persist`, together with any other pending writes.
///
/// When the source has a subpath, the content may have been built on a
/// [`read_sidecar`] that found neither file. The legacy path is then
/// staged as [`RepoBackend::expect_still_absent`] first, so a legacy file
/// that only appears when `persist` creates the branch from base turns the
/// save into a conflict instead of a root file that hides it.
pub async fn write_sidecar(backend: &dyn RepoBackend, law_id: &str, content: &str) -> Result<()> {
    let path = sidecar_path(law_id)?;
    if backend.repo_subpath().is_some() {
        backend.expect_still_absent(&path).await?;
    }
    backend.write_repo_file(&path, content).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_path_is_repo_root_relative() {
        assert_eq!(
            sidecar_path("wet_voorbeeld").unwrap(),
            PathBuf::from("annotations/wet_voorbeeld/annotations.yaml")
        );
    }

    #[test]
    fn sidecar_path_rejects_anything_but_one_segment() {
        for bad in ["", ".", "..", "../x", "a/b", "a\\b", "/abs", "x\0y"] {
            assert!(sidecar_path(bad).is_err(), "accepted {bad:?}");
        }
    }
}
