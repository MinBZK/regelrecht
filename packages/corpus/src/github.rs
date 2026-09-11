//! GitHub **domain** layer for the corpus: turns a [`GitHubSource`] into the
//! set of regulation files to load, on top of the shared
//! [`regelrecht_github::GithubClient`] transport.
//!
//! All raw REST talk (Trees/Contents/archive requests, ETag caching, rate
//! limits) lives in `regelrecht-github`. This module keeps only the
//! corpus-specific knowledge: which YAML paths under a source are laws, how to
//! pick the best version per law, and how to extract `implements` lists from a
//! repo archive. Everything here is a free function taking `&GithubClient`, so
//! there is no second GitHub client type and no circular dependency (the crate
//! never learns about corpus types).

use std::collections::{HashMap, HashSet};

use regelrecht_github::GithubClient;

use crate::error::{CorpusError, Result};
use crate::models::GitHubSource;

/// Result of fetching a GitHub source.
#[derive(Debug)]
pub enum FetchResult {
    /// New or updated content was fetched.
    Fetched(Vec<FetchedFile>),
    /// Content has not changed since last fetch (HTTP 304).
    NotModified,
}

/// A fetched file from GitHub.
#[derive(Debug, Clone)]
pub struct FetchedFile {
    pub path: String,
    pub content: String,
}

/// A YAML file discovered via the Trees API: its repo-relative path plus the
/// blob sha the tree listing reported. The sha is the file's content identity
/// — two listings reporting the same sha are byte-identical.
#[derive(Debug, Clone)]
struct TreeFile {
    path: String,
    sha: Option<String>,
}

/// Fetch only laws matching the given `$id` set from a GitHub source.
///
/// Uses the Trees API (1 call) to discover file paths, matches them against
/// `law_ids` by extracting the law directory name from the path
/// (`{base}/{layer}/{law_id}/{date}.yaml`), picks the best version per law
/// (latest `valid_from` ≤ today), and fetches only those files.
pub async fn fetch_source_filtered(
    client: &GithubClient,
    source: &GitHubSource,
    token: Option<&str>,
    law_ids: &HashSet<String>,
    today: &str,
) -> Result<FetchResult> {
    if law_ids.is_empty() {
        return Ok(FetchResult::Fetched(Vec::new()));
    }

    let base_path = source.path.as_deref().unwrap_or("");

    let all_paths = match list_yaml_files(
        client,
        &source.full_repo(),
        source.effective_ref(),
        base_path,
        token,
    )
    .await?
    {
        Some(paths) => paths,
        None => return Ok(FetchResult::NotModified),
    };

    let best_per_law = group_best_versions(&all_paths, base_path, Some(law_ids), today);

    tracing::info!(
        matched = best_per_law.len(),
        requested = law_ids.len(),
        "fetching filtered laws from GitHub"
    );

    let mut files = Vec::new();
    for file in best_per_law.values() {
        match client
            .fetch_file_raw(
                &source.full_repo(),
                source.effective_ref(),
                &file.path,
                token,
            )
            .await
        {
            Ok(content) => files.push(FetchedFile {
                path: file.path.clone(),
                content,
            }),
            Err(e) => {
                tracing::warn!(path = %file.path, error = %e, "Failed to fetch file, skipping");
            }
        }
    }

    Ok(FetchResult::Fetched(files))
}

/// Enumerate every law in a source via the Trees API (1 call), selecting the
/// best version per law — WITHOUT fetching any file content. Returns
/// `(law_id, repo_path, blob_sha)` triples; the sha is the file's content
/// identity from the tree listing, so callers can detect content change across
/// enumerations without fetching bodies. This is the cheap enumeration the
/// lightweight corpus index is built from.
pub async fn list_source_law_paths(
    client: &GithubClient,
    source: &GitHubSource,
    token: Option<&str>,
    today: &str,
) -> Result<Vec<(String, String, Option<String>)>> {
    let base_path = source.path.as_deref().unwrap_or("");
    let all_paths = match list_yaml_files(
        client,
        &source.full_repo(),
        source.effective_ref(),
        base_path,
        token,
    )
    .await?
    {
        Some(paths) => paths,
        None => return Ok(Vec::new()),
    };
    Ok(group_best_versions(&all_paths, base_path, None, today)
        .into_iter()
        .map(|(law_id, file)| (law_id, file.path, file.sha))
        .collect())
}

/// Bulk `implements` scan via the repo archive: download the tarball in one
/// request (through the shared client) and return each YAML law's `implements`
/// list — `(repo-relative path, implements)` pairs for `.yaml`/`.yml` files,
/// the archive's top-level `{owner}-{repo}-{sha}/` component stripped.
///
/// Bodies are parsed and DISCARDED one at a time during extraction (see
/// [`extract_implements_from_tar_gz`]) so a large corpus archive never
/// materialises in memory at once. gunzip + untar + parse are synchronous and
/// CPU-bound, so they run on a blocking thread off the async runtime.
pub async fn fetch_archive_implements(
    client: &GithubClient,
    repo: &str,
    git_ref: &str,
    token: Option<&str>,
) -> Result<Vec<(String, Vec<String>)>> {
    let bytes = client.fetch_tarball(repo, git_ref, token).await?;
    let files = tokio::task::spawn_blocking(move || extract_implements_from_tar_gz(bytes.as_ref()))
        .await
        .map_err(|e| CorpusError::Config(format!("archive extract task panicked: {e}")))??;
    Ok(files)
}

/// Download a repo at `git_ref` and unpack it into `dest` — one archive
/// request, the whole tree on disk.
///
/// The sibling of [`fetch_archive_implements`]: same single tarball, but the
/// bodies are written out instead of parsed and dropped. For a consumer that
/// needs the files *as files* rather than as an index — a regulation root the
/// engine walks itself, for instance, where every version of every law has to
/// be present and the Trees API's "best version per law" is the wrong answer.
///
/// `dest` must exist and is written into directly: the archive's single
/// top-level `{owner}-{repo}-{sha}/` component is stripped, so `dest` ends up
/// holding the repo's own top level. An entry that would land outside `dest`
/// (an absolute path, or one climbing out with `..`) fails the whole unpack
/// rather than being skipped silently — a tarball is remote input, and a
/// traversal is the one thing that must never be forgiven halfway.
///
/// gunzip, untar and the writes are synchronous and IO/CPU-bound, so they run
/// on a blocking thread off the async runtime.
pub async fn fetch_archive_to_dir(
    client: &GithubClient,
    repo: &str,
    git_ref: &str,
    token: Option<&str>,
    dest: &std::path::Path,
) -> Result<()> {
    let bytes = client.fetch_tarball(repo, git_ref, token).await?;
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || unpack_tar_gz(bytes.as_ref(), &dest))
        .await
        .map_err(|e| CorpusError::Config(format!("archive unpack task panicked: {e}")))?
}

/// Unpack a gzipped tar into `dest`, stripping the archive's single top-level
/// directory component. See [`fetch_archive_to_dir`] for the contract.
fn unpack_tar_gz(bytes: &[u8], dest: &std::path::Path) -> Result<()> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| CorpusError::Git(format!("failed to read archive entries: {e}")))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|e| CorpusError::Git(format!("failed to read archive entry: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| CorpusError::Git(format!("archive entry has no path: {e}")))?
            .to_path_buf();
        let Some(relative) = strip_top_level(&path) else {
            continue;
        };
        let target = safe_join(dest, &relative)?;
        match entry.header().entry_type() {
            tar::EntryType::Directory => {
                std::fs::create_dir_all(&target)?;
            }
            tar::EntryType::Regular => {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                entry.unpack(&target).map_err(|e| {
                    CorpusError::Git(format!("failed to write {}: {e}", target.display()))
                })?;
            }
            // Symlinks, hardlinks and devices: a regulation corpus has none,
            // and unpacking them is how an archive reaches outside `dest`
            // without any component of its own path saying so.
            other => {
                tracing::debug!(path = %relative.display(), kind = ?other, "archive entry is not a file or directory; skipping");
            }
        }
    }
    Ok(())
}

/// Drop the archive's single top-level directory component. `None` for the
/// top-level entry itself (nothing left to write).
fn strip_top_level(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut components = path.components();
    components.next()?;
    let rest = components.as_path();
    (!rest.as_os_str().is_empty()).then(|| rest.to_path_buf())
}

/// Join `relative` onto `dest`, refusing anything that would leave `dest`.
///
/// Checked component by component instead of after the fact: a
/// `canonicalize` of a path that does not exist yet cannot answer the
/// question, and a prefix comparison on strings is one symlink away from
/// being wrong.
fn safe_join(dest: &std::path::Path, relative: &std::path::Path) -> Result<std::path::PathBuf> {
    use std::path::Component;
    let mut target = dest.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => target.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(CorpusError::Git(format!(
                    "archive entry {} points outside the destination directory",
                    relative.display()
                )));
            }
        }
    }
    Ok(target)
}

/// List YAML files under `base_path` in a repo tree via the shared client's
/// Trees call, keeping the blob sha each entry reported. Returns `None` on a
/// 304 (tree unchanged). Narrows the crate's blob listing to `.yaml` files
/// inside `base_path`.
async fn list_yaml_files(
    client: &GithubClient,
    repo: &str,
    git_ref: &str,
    base_path: &str,
    token: Option<&str>,
) -> Result<Option<Vec<TreeFile>>> {
    let entries = match client.list_tree_files(repo, git_ref, token).await? {
        Some(entries) => entries,
        None => return Ok(None),
    };

    let yaml_files: Vec<TreeFile> = entries
        .into_iter()
        .filter(|e| {
            e.path.ends_with(".yaml")
                && (base_path.is_empty()
                    || e.path == base_path
                    || e.path.starts_with(&format!("{}/", base_path)))
        })
        .map(|e| TreeFile {
            path: e.path,
            sha: e.sha,
        })
        .collect();

    tracing::debug!(repo = %repo, count = yaml_files.len(), "Found YAML files in tree");
    Ok(Some(yaml_files))
}

/// Group repo-relative YAML files by `law_id` (the directory name), keeping the
/// best version per law (closest valid date ≤ today, else latest). `filter`,
/// when set, restricts to those law_ids. Path format:
/// `{base_path}/{layer}/{law_id}/{date}.yaml`.
fn group_best_versions(
    all_paths: &[TreeFile],
    base_path: &str,
    filter: Option<&HashSet<String>>,
    today: &str,
) -> HashMap<String, TreeFile> {
    let prefix = if base_path.is_empty() {
        String::new()
    } else {
        format!("{}/", base_path)
    };
    let mut best_per_law: HashMap<String, TreeFile> = HashMap::new();

    for file in all_paths {
        let path = &file.path;
        let rel = if prefix.is_empty() {
            path.as_str()
        } else {
            match path.strip_prefix(&prefix) {
                Some(r) => r,
                None => continue,
            }
        };

        let parts: Vec<&str> = rel.split('/').collect();
        if parts.len() < 3 {
            continue;
        }

        // Annotations are persisted at the reserved
        // `annotations/{law_id}/annotations.yaml` path in a traject's own repo.
        // That shape collides with the law-file convention
        // `{layer}/{law_id}/{date}.yaml`, so without this guard the annotation
        // file is indexed as a phantom law whose body is the annotation YAML —
        // the law then opens to an empty editor ("Geen items"). Skip the
        // annotations subtree entirely.
        if parts[0] == "annotations" {
            continue;
        }

        let law_id = parts[parts.len() - 2];
        if let Some(f) = filter {
            if !f.contains(law_id) {
                continue;
            }
        }

        let new_date = crate::source_map::extract_date_from_path(rel);

        if let Some(existing) = best_per_law.get(law_id) {
            let existing_date = crate::source_map::extract_date_from_path(&existing.path);

            let new_wins = crate::source_map::pick_best_version(
                existing_date.as_deref(),
                new_date.as_deref(),
                today,
            );

            if new_wins {
                best_per_law.insert(law_id.to_string(), file.clone());
            }
        } else {
            best_per_law.insert(law_id.to_string(), file.clone());
        }
    }

    best_per_law
}

/// Stream a gzipped tar produced by GitHub's tarball endpoint and return
/// `(repo-relative path, implements list)` for every `.yaml`/`.yml` file. The
/// archive nests everything under a single top-level `{owner}-{repo}-{sha}/`
/// directory; that first component is stripped. Directories, non-YAML files,
/// and non-UTF-8 bodies are skipped rather than failing the whole scan.
///
/// Each body is read into a scratch `String`, parsed for `implements`, and
/// dropped before the next entry — so peak memory is one law body plus the
/// (tiny) implements result, never the whole decompressed corpus.
fn extract_implements_from_tar_gz(bytes: &[u8]) -> Result<Vec<(String, Vec<String>)>> {
    use std::io::Read;
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    let mut out = Vec::new();
    let entries = archive
        .entries()
        .map_err(|e| CorpusError::Git(format!("failed to read archive entries: {e}")))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|e| CorpusError::Git(format!("failed to read archive entry: {e}")))?;
        if entry.header().entry_type() != tar::EntryType::Regular {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| CorpusError::Git(format!("archive entry has no path: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");
        // Strip the archive's single top-level directory component.
        let Some((_, rel)) = path.split_once('/') else {
            continue;
        };
        if !(rel.ends_with(".yaml") || rel.ends_with(".yml")) {
            continue;
        }
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_err() {
            tracing::debug!(path = %rel, "archive entry is not valid UTF-8; skipping");
            continue;
        }
        let implements = crate::source_map::collect_law_implements(&content);
        out.push((rel.to_string(), implements));
        // `content` dropped here — bodies never accumulate.
    }
    Ok(out)
}

#[cfg(test)]
mod unpack_tests {
    use super::unpack_tar_gz;
    use std::io::Write;

    /// Build a gzipped tar from `(path, body)` pairs, exactly as GitHub's
    /// tarball endpoint does: everything under one top-level directory.
    fn tar_gz(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, body) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, body.as_bytes())
                .expect("tar entry moet te schrijven zijn");
        }
        let tar = builder.into_inner().expect("tar moet af te ronden zijn");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&tar).expect("gzip moet te schrijven zijn");
        gz.finish().expect("gzip moet af te ronden zijn")
    }

    /// The whole tree lands on disk, the archive's `{owner}-{repo}-{sha}/`
    /// wrapper stripped — and *every* version of a law is there. That last
    /// part is the reason this function exists next to the Trees-API path,
    /// which keeps one version per law: an engine that picks its version on a
    /// moment needs all of them.
    #[test]
    fn unpack_strips_the_top_level_and_keeps_every_version() {
        let archive = tar_gz(&[
            ("owner-repo-abc123/README.md", "hoi"),
            ("owner-repo-abc123/regulation/nl/wet/w/2024-01-01.yaml", "a"),
            ("owner-repo-abc123/regulation/nl/wet/w/2025-01-01.yaml", "b"),
        ]);
        let dest = tempfile::tempdir().expect("tempdir");

        unpack_tar_gz(&archive, dest.path()).expect("unpack moet slagen");

        let laws = dest.path().join("regulation/nl/wet/w");
        assert_eq!(
            std::fs::read_to_string(laws.join("2024-01-01.yaml")).ok(),
            Some("a".to_string())
        );
        assert_eq!(
            std::fs::read_to_string(laws.join("2025-01-01.yaml")).ok(),
            Some("b".to_string())
        );
        assert!(dest.path().join("README.md").is_file());
    }

    /// A tarball is remote input. An entry that climbs out of the destination,
    /// or names an absolute path, is refused — and refused as an error, not
    /// skipped: half an unpacked corpus is a state nobody should have to
    /// reason about, and a write outside the temp directory is a state nobody
    /// should have at all.
    ///
    /// Asserted on the guard rather than through a crafted archive, because
    /// `tar::Builder` refuses to *write* a `..` path at all — the attack only
    /// arrives in bytes produced elsewhere.
    #[test]
    fn join_refuses_a_path_that_leaves_the_destination() {
        let dest = std::path::Path::new("/tmp/bestemming");
        for evil in [
            "../buiten.yaml",
            "regulation/../../buiten.yaml",
            "/etc/passwd",
        ] {
            let err = super::safe_join(dest, std::path::Path::new(evil))
                .expect_err("{evil} moet geweigerd worden");
            assert!(
                err.to_string().contains("outside the destination"),
                "verwachtte een traversal-melding voor {evil}, kreeg {err}"
            );
        }
    }

    /// The other half of the same guard: a plain nested path is joined as-is,
    /// and `./` is a no-op rather than a refusal.
    #[test]
    fn join_keeps_a_plain_nested_path() {
        let joined = super::safe_join(
            std::path::Path::new("/tmp/bestemming"),
            std::path::Path::new("./regulation/nl/wet/w/2024-01-01.yaml"),
        )
        .expect("een gewoon pad moet gewoon samengevoegd worden");
        assert_eq!(
            joined,
            std::path::Path::new("/tmp/bestemming/regulation/nl/wet/w/2024-01-01.yaml")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{group_best_versions, TreeFile};
    use std::collections::HashMap;

    fn sorted_ids(map: &HashMap<String, TreeFile>) -> Vec<String> {
        let mut ids: Vec<String> = map.keys().cloned().collect();
        ids.sort();
        ids
    }

    // A saved annotation lives at `annotations/{law_id}/annotations.yaml` in
    // the traject's own repo. That path shape collides with the law-file
    // convention `{layer}/{law_id}/{date}.yaml`, so without an explicit guard
    // the indexer registers the annotation file as a phantom law whose
    // "content" is the annotation YAML — the law then opens to an empty editor
    // ("Geen items"). Annotations must never be indexed as laws.
    #[test]
    fn annotation_files_are_not_indexed_as_laws() {
        let paths = vec![
            TreeFile {
                path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2026-01-01.yaml".to_string(),
                sha: Some("abc123".to_string()),
            },
            TreeFile {
                path: "annotations/zorgtoeslagwet/annotations.yaml".to_string(),
                sha: Some("def456".to_string()),
            },
        ];
        let best = group_best_versions(&paths, "", None, "2026-06-01");
        assert_eq!(sorted_ids(&best), vec!["wet_op_de_zorgtoeslag".to_string()]);
        assert!(
            !best.contains_key("zorgtoeslagwet"),
            "annotation file was mis-indexed as law 'zorgtoeslagwet'"
        );
    }

    // Both routes into `pick_best_version` must read a filename the same way.
    // The tree route used to hand it the bare stem, so `2025-01-01-concept`
    // sorted above `2025-01-01` and the concept won — while the local scan,
    // which validates the stem as a date, dropped it. Same repo, two answers.
    #[test]
    fn undated_stem_loses_to_a_dated_file_on_both_routes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let law_dir = dir.path().join("wet").join("test_wet");
        std::fs::create_dir_all(&law_dir).expect("create law dir");
        let body = "$id: test_wet\nname: Test\nregulatory_layer: WET\narticles: []\n";
        for stem in ["2025-01-01", "2025-01-01-concept"] {
            std::fs::write(law_dir.join(format!("{stem}.yaml")), body).expect("write law");
        }

        let source = crate::models::Source {
            id: "local".to_string(),
            name: "Local".to_string(),
            source_type: crate::models::SourceType::Local {
                local: crate::models::LocalSource {
                    path: dir.path().to_path_buf(),
                },
            },
            scopes: vec![],
            priority: 1,
            auth_ref: None,
            strict_auth: false,
        };
        let mut map = crate::source_map::SourceMap::new("2026-06-01");
        map.load_source(&source).expect("load local source");
        let local_pick = map
            .get_law("test_wet")
            .expect("law loaded")
            .file_path
            .clone();

        let paths: Vec<TreeFile> = ["2025-01-01", "2025-01-01-concept"]
            .iter()
            .map(|stem| TreeFile {
                path: format!("wet/test_wet/{stem}.yaml"),
                sha: None,
            })
            .collect();
        let tree_pick = group_best_versions(&paths, "", None, "2026-06-01")
            .get("test_wet")
            .expect("law indexed")
            .path
            .clone();

        assert!(
            local_pick.ends_with("2025-01-01.yaml"),
            "local scan picked {local_pick}"
        );
        assert!(
            tree_pick.ends_with("2025-01-01.yaml"),
            "tree scan picked {tree_pick}"
        );
    }
}
