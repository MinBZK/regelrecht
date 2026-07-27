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

/// Fetch all YAML regulation files from a GitHub source.
///
/// Returns [`FetchResult::NotModified`] when the tree has not changed (HTTP
/// 304) so callers can preserve previously loaded data.
pub async fn fetch_source(
    client: &GithubClient,
    source: &GitHubSource,
    token: Option<&str>,
) -> Result<FetchResult> {
    let base_path = source.path.as_deref().unwrap_or("");

    let yaml_paths = match list_yaml_files(
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

    if yaml_paths.is_empty() {
        return Ok(FetchResult::Fetched(Vec::new()));
    }

    let mut files = Vec::new();
    for file in &yaml_paths {
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

    let best_per_law = group_best_versions(&all_paths, base_path, Some(law_ids));

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
    Ok(group_best_versions(&all_paths, base_path, None)
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
) -> HashMap<String, TreeFile> {
    let prefix = if base_path.is_empty() {
        String::new()
    } else {
        format!("{}/", base_path)
    };
    let today = crate::source_map::today_str();
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

        let filename = parts[parts.len() - 1];
        let new_date = filename.strip_suffix(".yaml");

        if let Some(existing) = best_per_law.get(law_id) {
            let existing_filename = existing.path.rsplit('/').next().unwrap_or("");
            let existing_date = existing_filename.strip_suffix(".yaml");

            let new_wins = crate::source_map::pick_best_version(existing_date, new_date, &today);

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
        let best = group_best_versions(&paths, "", None);
        assert_eq!(sorted_ids(&best), vec!["wet_op_de_zorgtoeslag".to_string()]);
        assert!(
            !best.contains_key("zorgtoeslagwet"),
            "annotation file was mis-indexed as law 'zorgtoeslagwet'"
        );
    }
}
