//! Rule resolver for cross-law lookups
//!
//! Provides indexing and lookup functionality for laws, including:
//! - Law registry by ID with multi-version support
//! - Output index for fast article lookup by output name
//! - Implements index for IoC open term resolution
//! - Version selection based on reference_date
//!
//! # Multi-version Support
//!
//! Laws can have multiple versions with different `valid_from` dates. When looking up
//! a law, you can optionally provide a `reference_date` to select the appropriate version:
//! - Versions where `valid_from <= reference_date` are considered valid
//! - The version with the most recent `valid_from` among valid versions is selected
//! - If no `reference_date` is provided, the most recent version is used
//!
//! # Security
//!
//! The resolver enforces a maximum number of loaded laws (see [`crate::config::MAX_LOADED_LAWS`])
//! to prevent memory exhaustion attacks.

use crate::article::{Article, ArticleBasedLaw, HookFilter, HookPoint, ProcedureDefinition};
use crate::config;
use crate::error::{EngineError, Result};
use crate::priority::{self, Candidate};
use crate::types::Value;
use chrono::NaiveDate;
use std::collections::HashMap;

/// A reference to a law article, used in implements and overrides indexes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LawArticleRef {
    pub(crate) law_id: String,
    pub(crate) article_number: String,
}

/// A hook index entry linking a hook declaration to the law and article that defined it.
pub(crate) struct HookEntry {
    pub(crate) law_id: String,
    pub(crate) article_number: String,
    filter: HookFilter,
}

/// Resolves cross-law references and provides law registry functionality.
///
/// The resolver maintains several indexes for efficient lookups:
/// - **Law registry**: All loaded laws by ID, supporting multiple versions per ID
/// - **Output index**: Maps (law_id, output_name) to article number
/// - **Implements index**: Maps (law_id, article, open_term_id) to implementing regulations
///
/// # Multi-version Support
///
/// Multiple versions of the same law (same `$id`) can be loaded. Each version
/// has a `valid_from` date. When querying, provide a `reference_date` to get
/// the appropriate version:
///
/// ```ignore
/// // Load two versions of the same law
/// resolver.load_from_yaml(law_v1_yaml)?; // valid_from: 2024-01-01
/// resolver.load_from_yaml(law_v2_yaml)?; // valid_from: 2025-01-01
///
/// // Get version for a specific date
/// let law = resolver.get_law_for_date("my_law", Some(date!(2024, 6, 1)));
/// // Returns v1 (valid_from 2024-01-01)
/// ```
///
/// # Example
///
/// ```ignore
/// use regelrecht_engine::RuleResolver;
///
/// let mut resolver = RuleResolver::new();
/// resolver.load_from_yaml(yaml_str)?;
///
/// // Find article by output
/// let article = resolver.get_article_by_output("zorgtoeslagwet", "standaardpremie", None);
/// ```
pub struct RuleResolver {
    /// Registry of loaded laws by ID, supporting multiple versions per law ID.
    /// Each law ID maps to a list of versions, sorted by valid_from date (newest first).
    law_versions: HashMap<String, Vec<ArticleBasedLaw>>,
    /// Index: "law_id\0output_name" -> article_number
    /// Note: This index uses the most recent version of each law.
    /// Uses a flat string key (null-separated) to avoid two allocations per lookup.
    output_index: HashMap<String, String>,
    /// IoC index: (law_id, article, open_term_id) -> list of implementing articles
    implements_index: HashMap<(String, String, String), Vec<LawArticleRef>>,
    /// Hook index: (hook_point, legal_character) -> list of (law_id, article_number, filter)
    /// Enables O(1) lookup of hooks that should fire for a given lifecycle event.
    hooks_index: HashMap<(HookPoint, String), Vec<HookEntry>>,
    /// Override index: (target_law, target_article, output) -> list of overriding articles
    /// Enables O(1) lookup of lex specialis overrides for a given output.
    overrides_index: HashMap<(String, String, String), Vec<LawArticleRef>>,
    /// Procedure index: (legal_character, procedure_id) -> (procedure definition, defining_law_id)
    /// Loaded from laws that define `procedure:` (typically the AWB).
    procedure_index: HashMap<(String, String), (ProcedureDefinition, String)>,
    /// Maps legal_character -> default procedure_id for that character.
    procedure_defaults: HashMap<String, String>,
}

impl Default for RuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleResolver {
    /// Create a new empty resolver.
    pub fn new() -> Self {
        Self {
            law_versions: HashMap::new(),
            output_index: HashMap::new(),
            implements_index: HashMap::new(),
            hooks_index: HashMap::new(),
            overrides_index: HashMap::new(),
            procedure_index: HashMap::new(),
            procedure_defaults: HashMap::new(),
        }
    }

    /// Load a law into the resolver.
    ///
    /// If a law with the same ID and valid_from already exists, it will be replaced.
    /// Otherwise, the new version is added to the version list.
    ///
    /// # Arguments
    /// * `law` - The law to load
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` if the maximum number of laws would be exceeded.
    ///
    /// # Security
    ///
    /// Enforces [`config::MAX_LOADED_LAWS`] to prevent memory exhaustion.
    pub fn load_law(&mut self, law: ArticleBasedLaw) -> Result<()> {
        let law_id = law.id.clone();
        let valid_from = law.valid_from.clone();

        // Count total laws across all versions
        let total_laws: usize = self.law_versions.values().map(|v| v.len()).sum();

        // Check if we're replacing an existing version (which doesn't increase count)
        let is_replacement = self
            .law_versions
            .get(&law_id)
            .is_some_and(|versions| versions.iter().any(|v| v.valid_from == valid_from));

        // Enforce law count limit (applies to all new versions, not just new law IDs)
        if !is_replacement && total_laws >= config::MAX_LOADED_LAWS {
            tracing::warn!(
                current = total_laws,
                max = config::MAX_LOADED_LAWS,
                law_id = %law_id,
                "Maximum law count exceeded"
            );
            return Err(EngineError::LoadError(format!(
                "Maximum number of laws exceeded ({} laws)",
                config::MAX_LOADED_LAWS
            )));
        }

        // Get or create the version list for this law ID
        let versions = self.law_versions.entry(law_id.clone()).or_default();

        // Check if we're replacing an existing version (same valid_from)
        let existing_idx = versions.iter().position(|v| v.valid_from == valid_from);
        if let Some(idx) = existing_idx {
            tracing::debug!(law_id = %law_id, valid_from = ?valid_from, "Replacing existing version");
            versions[idx] = law;
        } else {
            tracing::debug!(law_id = %law_id, valid_from = ?valid_from, "Adding new version");
            versions.push(law);
        }

        // Sort versions by valid_from date (newest first)
        // Use sort_by_cached_key to parse dates once instead of per-comparison
        versions.sort_by_cached_key(|v| {
            std::cmp::Reverse(v.valid_from.as_ref().and_then(|s| parse_date(s).ok()))
        });

        // Rebuild indexes using the most recent version
        self.rebuild_indexes_for_law(&law_id);

        let total_laws: usize = self.law_versions.values().map(|v| v.len()).sum();
        tracing::debug!(law_id = %law_id, total = total_laws, "Law loaded");
        Ok(())
    }

    /// Load a law from YAML string.
    ///
    /// # Arguments
    /// * `yaml` - YAML content of the law
    ///
    /// # Returns
    /// The law ID on success.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - YAML parsing fails
    /// - Maximum number of laws would be exceeded
    pub fn load_from_yaml(&mut self, yaml: &str) -> Result<String> {
        let law = ArticleBasedLaw::from_yaml_str(yaml)?;
        let law_id = law.id.clone();
        self.load_law(law)?;
        Ok(law_id)
    }

    /// Get a law by ID (returns the most recent version).
    ///
    /// This is a convenience method that returns the most recent version.
    /// For version-aware lookups, use [`Self::get_law_for_date`].
    pub fn get_law(&self, law_id: &str) -> Option<&ArticleBasedLaw> {
        self.law_versions
            .get(law_id)
            .and_then(|versions| versions.first())
    }

    /// Get a law by ID for a specific reference date.
    ///
    /// Selects the appropriate version based on the reference date:
    /// - Versions where `valid_from <= reference_date` are considered valid
    /// - The version with the most recent `valid_from` among valid versions is returned
    /// - If `reference_date` is None, returns the most recent version
    ///
    /// # Arguments
    /// * `law_id` - The law identifier
    /// * `reference_date` - Optional date to select the appropriate version
    ///
    /// # Returns
    /// Reference to the selected version, or None if no valid version exists.
    pub fn get_law_for_date(
        &self,
        law_id: &str,
        reference_date: Option<NaiveDate>,
    ) -> Option<&ArticleBasedLaw> {
        let versions = self.law_versions.get(law_id)?;

        match reference_date {
            None => versions.first(), // Return most recent
            Some(ref_date) => self.select_version_for_date(versions, ref_date),
        }
    }

    /// Select the appropriate version for a reference date.
    ///
    /// # Selection Logic
    /// 1. Filter versions where `valid_from <= reference_date`
    /// 2. Return the version with the most recent `valid_from`
    /// 3. If no version has `valid_from`, return the most recent overall
    fn select_version_for_date<'a>(
        &self,
        versions: &'a [ArticleBasedLaw],
        reference_date: NaiveDate,
    ) -> Option<&'a ArticleBasedLaw> {
        // Return the first valid version (already sorted newest first)
        versions.iter().find(|v| {
            v.valid_from
                .as_ref()
                .and_then(|s| parse_date(s).ok())
                .is_none_or(|valid_from| valid_from <= reference_date)
        })
    }

    /// Get an article by law ID and output name.
    ///
    /// # Arguments
    /// * `law_id` - The law identifier
    /// * `output` - The output name to find
    /// * `reference_date` - Optional date to select the appropriate law version
    ///
    /// # Returns
    /// Reference to the article if found.
    pub fn get_article_by_output(
        &self,
        law_id: &str,
        output: &str,
        reference_date: Option<NaiveDate>,
    ) -> Option<&Article> {
        let law = self.get_law_for_date(law_id, reference_date)?;
        // Try indexed lookup first (O(1)), fall back to linear scan
        let index_key = format!("{}\0{}", law_id, output);
        if let Some(article_number) = self.output_index.get(&index_key) {
            if let Some(article) = law.find_article_by_number(article_number) {
                // Verify the article in this version actually has the output
                if article.has_output(output) {
                    return Some(article);
                }
            }
        }
        // Fallback: linear scan (handles version-specific differences)
        law.find_article_by_output(output)
    }

    /// Find all implementations of an open term, resolved by priority.
    ///
    /// Check if a law's scope fields match the execution scope.
    ///
    /// Scope fields are law-level metadata that limit territorial applicability
    /// (e.g., `gemeente_code`, `provincie_code`). A law with no scope fields
    /// is considered national and always matches. A law with scope fields only
    /// matches if every scope field has a matching value in the execution scope.
    fn matches_scope(law: &ArticleBasedLaw, scope: &HashMap<String, Value>) -> bool {
        if let Some(ref law_gemeente) = law.gemeente_code {
            let scope_value = scope.get("gemeente_code").and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            });
            match scope_value {
                Some(sg) if sg == law_gemeente => {}
                _ => return false,
            }
        }
        if let Some(ref law_waterschap) = law.waterschap_code {
            let scope_value = scope.get("waterschap_code").and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            });
            match scope_value {
                Some(sw) if sw == law_waterschap => {}
                _ => return false,
            }
        }
        true
    }

    /// Looks up the implements index for regulations that declare they fill
    /// the given open term. Optionally filters by temporal validity.
    ///
    /// Returns candidates sorted by priority (winner first), along with
    /// each candidate's (law, article) pair.
    ///
    /// # Arguments
    /// * `law_id` - The law that declares the open term
    /// * `article` - The article number that declares the open term
    /// * `open_term_id` - The open term identifier
    /// * `reference_date` - Optional date to filter by temporal validity
    pub fn find_implementations(
        &self,
        law_id: &str,
        article: &str,
        open_term_id: &str,
        reference_date: Option<NaiveDate>,
        scope: &HashMap<String, Value>,
    ) -> Result<Vec<(&ArticleBasedLaw, &Article)>> {
        let key = (
            law_id.to_string(),
            article.to_string(),
            open_term_id.to_string(),
        );
        let candidate_entries = match self.implements_index.get(&key) {
            Some(entries) => entries,
            None => return Ok(Vec::new()),
        };

        tracing::debug!(
            law_id = %law_id,
            article = %article,
            open_term_id = %open_term_id,
            candidates = candidate_entries.len(),
            "Finding implementations for open term"
        );

        // Resolve each candidate to actual (law, article) references
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut resolved: Vec<(&ArticleBasedLaw, &Article)> = Vec::new();

        for entry in candidate_entries {
            let Some(law) = self.get_law_for_date(&entry.law_id, reference_date) else {
                continue;
            };

            // Scope filtering: check all scope fields on the candidate law against
            // the execution parameters. A scoped regulation (e.g., with gemeente_code
            // or provincie_code) only matches when the execution scope contains the
            // same value. Unscoped regulations (national) always match.
            if !Self::matches_scope(law, scope) {
                tracing::debug!(
                    candidate = %entry.law_id,
                    "Skipping: scope fields do not match execution parameters"
                );
                continue;
            }

            let Some(art) = law
                .articles
                .iter()
                .find(|a| a.number == entry.article_number)
            else {
                continue;
            };

            candidates.push(Candidate {
                law,
                article_number: entry.article_number.clone(),
            });
            resolved.push((law, art));
        }

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Use priority resolution to sort — return winner first
        if let Some((winner_law, reason)) = priority::resolve_candidate(&candidates)? {
            tracing::debug!(
                winner = %winner_law.id,
                reason = %reason,
                "Open term implementation resolved"
            );

            // Put winner first, then the rest
            let winner_idx = resolved.iter().position(|(law, _)| law.id == winner_law.id);
            if let Some(idx) = winner_idx {
                if idx != 0 {
                    resolved.swap(0, idx);
                }
            }
        }

        Ok(resolved)
    }

    /// Get the number of entries in the implements index.
    #[cfg(test)]
    pub fn implements_count(&self) -> usize {
        self.implements_index.values().map(|v| v.len()).sum()
    }

    /// Get the number of entries in the output index.
    ///
    /// This counts the total number of (law_id, output_name) pairs across all laws.
    pub fn output_count(&self) -> usize {
        self.output_index.len()
    }

    /// List all (law_id, output_name) pairs from the output index.
    pub fn list_all_outputs(&self) -> Vec<(&str, &str)> {
        let mut outputs: Vec<(&str, &str)> = self
            .output_index
            .keys()
            .filter_map(|key| key.split_once('\0'))
            .collect();
        outputs.sort();
        outputs
    }

    /// Load all YAML law files from a directory (recursively).
    ///
    /// Scans the given directory for `.yaml` files and loads each one.
    /// Files that fail to parse are logged as warnings and skipped.
    ///
    /// # Arguments
    /// * `dir` - Path to the directory to scan
    ///
    /// # Returns
    /// Number of successfully loaded law files.
    ///
    /// # Errors
    /// Returns error if the directory cannot be read.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_directory(&mut self, dir: &std::path::Path) -> Result<usize> {
        let mut count = 0;
        self.load_from_directory_recursive(dir, &mut count)?;
        Ok(count)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_directory_recursive(
        &mut self,
        dir: &std::path::Path,
        count: &mut usize,
    ) -> Result<()> {
        use std::fs;

        let entries = fs::read_dir(dir).map_err(|e| {
            EngineError::LoadError(format!(
                "Failed to read directory '{}': {}",
                dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                EngineError::LoadError(format!("Failed to read directory entry: {}", e))
            })?;
            let path = entry.path();

            if path.is_dir() {
                self.load_from_directory_recursive(&path, count)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                match ArticleBasedLaw::from_yaml_file(&path) {
                    Ok(law) => match self.load_law(law) {
                        Ok(()) => {
                            *count += 1;
                        }
                        Err(e) => {
                            tracing::warn!(
                                path = %path.display(),
                                error = %e,
                                "Failed to register law from file"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to parse YAML law file"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// List all loaded law IDs (unique, not including versions).
    pub fn list_laws(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.law_versions.keys().map(|s| s.as_str()).collect();
        ids.sort();
        ids
    }

    /// Get the number of unique law IDs (not counting versions).
    pub fn law_count(&self) -> usize {
        self.law_versions.len()
    }

    /// Get the total number of loaded law versions.
    pub fn version_count(&self) -> usize {
        self.law_versions.values().map(|v| v.len()).sum()
    }

    /// Get the number of versions for a specific law.
    pub fn version_count_for_law(&self, law_id: &str) -> usize {
        self.law_versions.get(law_id).map(|v| v.len()).unwrap_or(0)
    }

    /// Check if a law is loaded (any version).
    pub fn has_law(&self, law_id: &str) -> bool {
        self.law_versions.contains_key(law_id)
    }

    /// Iterate over all loaded law versions (all IDs, all versions).
    pub fn all_law_versions(&self) -> impl Iterator<Item = &ArticleBasedLaw> {
        self.law_versions.values().flat_map(|v| v.iter())
    }

    /// Unload all versions of a law from the resolver.
    ///
    /// Removes all versions of the law and all its indexes.
    ///
    /// # Returns
    /// `true` if the law was removed, `false` if it didn't exist.
    pub fn unload_law(&mut self, law_id: &str) -> bool {
        if self.law_versions.remove(law_id).is_some() {
            self.remove_indexes_for_law(law_id);
            true
        } else {
            false
        }
    }

    /// Unload a specific version of a law.
    ///
    /// # Arguments
    /// * `law_id` - The law identifier
    /// * `valid_from` - The valid_from date of the version to remove
    ///
    /// # Returns
    /// `true` if the version was removed, `false` if it didn't exist.
    pub fn unload_law_version(&mut self, law_id: &str, valid_from: Option<&str>) -> bool {
        let Some(versions) = self.law_versions.get_mut(law_id) else {
            return false;
        };

        let original_len = versions.len();
        versions.retain(|v| v.valid_from.as_deref() != valid_from);

        if versions.len() < original_len {
            if versions.is_empty() {
                self.law_versions.remove(law_id);
                self.remove_indexes_for_law(law_id);
            } else {
                // Rebuild indexes with the new most recent version
                self.rebuild_indexes_for_law(law_id);
            }
            true
        } else {
            false
        }
    }

    /// Rebuild output, implements, hook, override, and procedure indexes for a specific law.
    fn rebuild_indexes_for_law(&mut self, law_id: &str) {
        // Remove old output index entries
        self.output_index
            .retain(|key, _| key.split_once('\0').is_none_or(|(id, _)| id != law_id));

        // Remove old implements index entries where this law is an implementor
        for candidates in self.implements_index.values_mut() {
            candidates.retain(|r| r.law_id != law_id);
        }
        self.implements_index.retain(|_, v| !v.is_empty());

        // Remove old hook index entries for this law
        for entries in self.hooks_index.values_mut() {
            entries.retain(|entry| entry.law_id != law_id);
        }
        self.hooks_index.retain(|_, v| !v.is_empty());

        // Remove old override index entries for this law
        for entries in self.overrides_index.values_mut() {
            entries.retain(|r| r.law_id != law_id);
        }
        self.overrides_index.retain(|_, v| !v.is_empty());

        // Remove old procedure index entries defined by this law
        self.procedure_index
            .retain(|_, (_, defining_law)| defining_law != law_id);
        // Clean up defaults whose backing procedure no longer exists
        let proc_index = &self.procedure_index;
        self.procedure_defaults
            .retain(|lc, proc_id| proc_index.contains_key(&(lc.clone(), proc_id.clone())));

        // Add new index entries from the most recent version
        // Access law_versions directly to avoid borrowing self through get_law()
        if let Some(versions) = self.law_versions.get(law_id) {
            if let Some(law) = versions.first() {
                // Procedure index (top-level)
                if let Some(procedures) = &law.procedure {
                    for proc_def in procedures {
                        let key = (
                            proc_def.applies_to.legal_character.clone(),
                            proc_def.id.clone(),
                        );
                        self.procedure_index
                            .insert(key, (proc_def.clone(), law_id.to_string()));

                        if proc_def.default.unwrap_or(false) {
                            self.procedure_defaults.insert(
                                proc_def.applies_to.legal_character.clone(),
                                proc_def.id.clone(),
                            );
                        }
                    }
                }

                for article in &law.articles {
                    // Output index
                    if let Some(exec) = article.get_execution_spec() {
                        if let Some(outputs) = &exec.output {
                            for output in outputs {
                                self.output_index.insert(
                                    format!("{}\0{}", law_id, output.name),
                                    article.number.clone(),
                                );
                            }
                        }
                    }

                    // Implements index (IoC)
                    if let Some(impl_decls) = article.get_implements() {
                        for decl in impl_decls {
                            let key = (
                                decl.law.clone(),
                                decl.article.clone(),
                                decl.open_term.clone(),
                            );
                            let entry = LawArticleRef {
                                law_id: law_id.to_string(),
                                article_number: article.number.clone(),
                            };
                            let candidates = self.implements_index.entry(key).or_default();
                            if !candidates.contains(&entry) {
                                candidates.push(entry);
                            }
                        }
                    }

                    // Hooks index
                    if let Some(hook_decls) = article.get_hooks() {
                        for decl in hook_decls {
                            if let Some(ref legal_char) = decl.applies_to.legal_character {
                                let key = (decl.hook_point, legal_char.clone());
                                let entry = HookEntry {
                                    law_id: law_id.to_string(),
                                    article_number: article.number.clone(),
                                    filter: decl.applies_to.clone(),
                                };
                                self.hooks_index.entry(key).or_default().push(entry);
                            }
                        }
                    }

                    // Overrides index
                    if let Some(ovr_decls) = article.get_overrides() {
                        for decl in ovr_decls {
                            let key = (decl.law.clone(), decl.article.clone(), decl.output.clone());
                            let entry = LawArticleRef {
                                law_id: law_id.to_string(),
                                article_number: article.number.clone(),
                            };
                            let candidates = self.overrides_index.entry(key).or_default();
                            if !candidates.contains(&entry) {
                                candidates.push(entry);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Remove all indexes for a law.
    fn remove_indexes_for_law(&mut self, law_id: &str) {
        // Remove output index entries
        self.output_index
            .retain(|key, _| key.split_once('\0').is_none_or(|(id, _)| id != law_id));

        // Remove from implements index (this law as implementor)
        for candidates in self.implements_index.values_mut() {
            candidates.retain(|r| r.law_id != law_id);
        }
        self.implements_index.retain(|_, v| !v.is_empty());

        // Remove hook index entries for this law
        for entries in self.hooks_index.values_mut() {
            entries.retain(|entry| entry.law_id != law_id);
        }
        self.hooks_index.retain(|_, v| !v.is_empty());

        // Remove override index entries for this law
        for entries in self.overrides_index.values_mut() {
            entries.retain(|r| r.law_id != law_id);
        }
        self.overrides_index.retain(|_, v| !v.is_empty());

        // Remove procedure index entries defined by this law
        self.procedure_index
            .retain(|_, (_, defining_law)| defining_law != law_id);
        let proc_index = &self.procedure_index;
        self.procedure_defaults
            .retain(|lc, proc_id| proc_index.contains_key(&(lc.clone(), proc_id.clone())));
    }

    /// Find hooks that match a given lifecycle event.
    ///
    /// Returns matching (law_id, article_number, filter) entries.
    /// Filters by stage: if the hook has a stage, it must match; if not, it defaults to "BESLUIT".
    pub(crate) fn find_hooks(
        &self,
        hook_point: HookPoint,
        legal_character: &str,
        decision_type: Option<&str>,
        stage: &str,
    ) -> Vec<&HookEntry> {
        let key = (hook_point, legal_character.to_string());
        let Some(entries) = self.hooks_index.get(&key) else {
            return Vec::new();
        };

        entries
            .iter()
            .filter(|entry| {
                // Stage filter: absent defaults to BESLUIT (backward compat per RFC-008)
                let hook_stage = entry.filter.stage.as_deref().unwrap_or("BESLUIT");
                if hook_stage != stage {
                    return false;
                }

                // Decision type filter: if specified, must match
                if let Some(ref filter_dt) = entry.filter.decision_type {
                    match decision_type {
                        Some(dt) if dt == filter_dt => {}
                        _ => return false,
                    }
                }

                true
            })
            .collect()
    }

    /// Find overrides for a specific article output.
    ///
    /// Returns matching (overriding_law_id, overriding_article_number) entries.
    pub(crate) fn find_overrides(
        &self,
        target_law: &str,
        target_article: &str,
        output: &str,
    ) -> &[LawArticleRef] {
        let key = (
            target_law.to_string(),
            target_article.to_string(),
            output.to_string(),
        );
        self.overrides_index
            .get(&key)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Find a procedure definition for a given legal character and optional procedure ID.
    ///
    /// If procedure_id is None, returns the default procedure for the legal character.
    pub fn find_procedure(
        &self,
        legal_character: &str,
        procedure_id: Option<&str>,
    ) -> Option<&ProcedureDefinition> {
        let proc_id = match procedure_id {
            Some(id) => id.to_string(),
            None => self.procedure_defaults.get(legal_character)?.clone(),
        };
        let key = (legal_character.to_string(), proc_id);
        self.procedure_index.get(&key).map(|(def, _)| def)
    }

    /// Validate that all override targets exist in loaded laws.
    ///
    /// Returns a list of validation errors for overrides that reference
    /// non-existent laws, articles, or outputs.
    pub fn validate_override_targets(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for ((target_law, target_article, target_output), overriders) in &self.overrides_index {
            // Check target law exists
            let Some(law) = self.get_law(target_law) else {
                for ovr in overriders {
                    errors.push(format!(
                        "Override {}:{} targets non-existent law '{target_law}'",
                        ovr.law_id, ovr.article_number
                    ));
                }
                continue;
            };

            // Check target article exists
            let Some(article) = law.find_article_by_number(target_article) else {
                for ovr in overriders {
                    errors.push(format!(
                        "Override {}:{} targets non-existent article '{target_article}' in '{target_law}'",
                        ovr.law_id, ovr.article_number
                    ));
                }
                continue;
            };

            // Check target output exists
            if !article.has_output(target_output) {
                for ovr in overriders {
                    errors.push(format!(
                        "Override {}:{} targets non-existent output '{target_output}' on '{target_law}:{target_article}'",
                        ovr.law_id, ovr.article_number
                    ));
                }
            }
        }
        errors
    }
}

/// Parse a date string in ISO 8601 format (YYYY-MM-DD).
fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| EngineError::InvalidOperation(format!("Failed to parse date '{}': {}", s, e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_law() -> &'static str {
        r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test article
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: 42
"#
    }

    fn make_test_law_with_valid_from(valid_from: &str, value: i32) -> String {
        format!(
            r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '{valid_from}'
articles:
  - number: '1'
    text: Test article version {valid_from}
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: {value}
"#
        )
    }

    #[test]
    fn test_resolver_basic() {
        let mut resolver = RuleResolver::new();

        let law_id = resolver.load_from_yaml(make_test_law()).unwrap();
        assert_eq!(law_id, "test_law");

        assert!(resolver.has_law("test_law"));
        assert!(!resolver.has_law("nonexistent"));
        assert_eq!(resolver.law_count(), 1);
    }

    #[test]
    fn test_resolver_get_law() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();

        let law = resolver.get_law("test_law").unwrap();
        assert_eq!(law.id, "test_law");
        assert_eq!(law.articles.len(), 1);
    }

    #[test]
    fn test_resolver_output_index() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();

        let article = resolver
            .get_article_by_output("test_law", "test_output", None)
            .unwrap();
        assert_eq!(article.number, "1");

        // Non-existent output
        assert!(resolver
            .get_article_by_output("test_law", "nonexistent", None)
            .is_none());

        // Non-existent law
        assert!(resolver
            .get_article_by_output("nonexistent", "test_output", None)
            .is_none());
    }

    #[test]
    fn test_resolver_list_laws() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_test_law()).unwrap();

        let laws = resolver.list_laws();
        assert_eq!(laws.len(), 1);
        assert_eq!(laws, vec!["test_law"]);
    }

    #[test]
    fn test_resolver_unload() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();

        assert!(resolver.has_law("test_law"));
        assert!(resolver.unload_law("test_law"));
        assert!(!resolver.has_law("test_law"));
        assert!(!resolver.unload_law("test_law")); // Already removed

        // Output index should also be removed
        assert!(resolver
            .get_article_by_output("test_law", "test_output", None)
            .is_none());
    }

    #[test]
    fn test_resolver_replace_law() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();

        // Load a different version of the same law (same valid_from = None)
        let updated_yaml = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: Updated article
    machine_readable:
      execution:
        output:
          - name: new_output
            type: number
        actions:
          - output: new_output
            value: 100
"#;
        resolver.load_from_yaml(updated_yaml).unwrap();

        // Should have the new article (replacement since same valid_from)
        let law = resolver.get_law("test_law").unwrap();
        assert_eq!(law.articles[0].number, "2");

        // Old output should be gone
        assert!(resolver
            .get_article_by_output("test_law", "test_output", None)
            .is_none());

        // New output should exist
        assert!(resolver
            .get_article_by_output("test_law", "new_output", None)
            .is_some());
    }

    #[test]
    fn test_resolver_law_count_limit() {
        // Test that we can't exceed the maximum law count
        // Note: This test uses a smaller limit to avoid long test times
        let mut resolver = RuleResolver::new();

        // Load a law to verify basic functionality
        resolver.load_from_yaml(make_test_law()).unwrap();
        assert_eq!(resolver.law_count(), 1);

        // Verify replacement doesn't count towards limit
        resolver.load_from_yaml(make_test_law()).unwrap();
        assert_eq!(resolver.law_count(), 1); // Should still be 1 (replacement)
    }

    #[test]
    fn test_resolver_load_law_returns_result() {
        // Test that load_law now returns a Result
        let mut resolver = RuleResolver::new();
        let law = ArticleBasedLaw::from_yaml_str(make_test_law()).unwrap();

        // First load should succeed
        assert!(resolver.load_law(law.clone()).is_ok());
        assert_eq!(resolver.law_count(), 1);

        // Replacement should also succeed
        assert!(resolver.load_law(law).is_ok());
        assert_eq!(resolver.law_count(), 1); // Still 1 - replacement
    }

    // -------------------------------------------------------------------------
    // Multi-version Support Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolver_multi_version_basic() {
        let mut resolver = RuleResolver::new();

        // Load two versions of the same law
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();

        // Should have 1 law with 2 versions
        assert_eq!(resolver.law_count(), 1);
        assert_eq!(resolver.version_count(), 2);
        assert_eq!(resolver.version_count_for_law("test_law"), 2);

        // get_law returns the most recent version
        let law = resolver.get_law("test_law").unwrap();
        assert_eq!(law.valid_from, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_resolver_get_law_for_date() {
        let mut resolver = RuleResolver::new();

        // Load three versions
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2023-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-06-01", 200))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 300))
            .unwrap();

        // Query for different dates
        let date_2023 = NaiveDate::from_ymd_opt(2023, 6, 1).unwrap();
        let date_2024 = NaiveDate::from_ymd_opt(2024, 12, 1).unwrap();
        let date_2025 = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();

        // 2023-06-01: Should get 2023-01-01 version
        let law = resolver
            .get_law_for_date("test_law", Some(date_2023))
            .unwrap();
        assert_eq!(law.valid_from, Some("2023-01-01".to_string()));

        // 2024-12-01: Should get 2024-06-01 version (most recent valid)
        let law = resolver
            .get_law_for_date("test_law", Some(date_2024))
            .unwrap();
        assert_eq!(law.valid_from, Some("2024-06-01".to_string()));

        // 2025-06-01: Should get 2025-01-01 version
        let law = resolver
            .get_law_for_date("test_law", Some(date_2025))
            .unwrap();
        assert_eq!(law.valid_from, Some("2025-01-01".to_string()));

        // None: Should get most recent version
        let law = resolver.get_law_for_date("test_law", None).unwrap();
        assert_eq!(law.valid_from, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_resolver_get_law_for_date_no_valid_version() {
        let mut resolver = RuleResolver::new();

        // Load a version valid from 2025
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 100))
            .unwrap();

        // Query for a date before any version is valid
        let date_2024 = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let law = resolver.get_law_for_date("test_law", Some(date_2024));
        assert!(law.is_none());
    }

    #[test]
    fn test_resolver_version_replacement() {
        let mut resolver = RuleResolver::new();

        // Load a version
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        assert_eq!(resolver.version_count(), 1);

        // Load the same version again (same valid_from) - should replace
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 200))
            .unwrap();
        assert_eq!(resolver.version_count(), 1); // Still 1, replaced

        // Load a different version - should add
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 300))
            .unwrap();
        assert_eq!(resolver.version_count(), 2);
    }

    #[test]
    fn test_resolver_unload_version() {
        let mut resolver = RuleResolver::new();

        // Load two versions
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();
        assert_eq!(resolver.version_count(), 2);

        // Unload one version
        assert!(resolver.unload_law_version("test_law", Some("2024-01-01")));
        assert_eq!(resolver.version_count(), 1);
        assert!(resolver.has_law("test_law"));

        // Unload remaining version
        assert!(resolver.unload_law_version("test_law", Some("2025-01-01")));
        assert_eq!(resolver.version_count(), 0);
        assert!(!resolver.has_law("test_law"));
    }

    #[test]
    fn test_resolver_article_by_output_with_date() {
        let mut resolver = RuleResolver::new();

        // Load two versions with different article numbers
        let v1 = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Article v1
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: 100
"#;
        let v2 = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '2'
    text: Article v2
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: 200
"#;
        resolver.load_from_yaml(v1).unwrap();
        resolver.load_from_yaml(v2).unwrap();

        let date_2024 = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let date_2025 = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();

        // Get article for 2024 - should get v1 article
        let article = resolver
            .get_article_by_output("test_law", "test_output", Some(date_2024))
            .unwrap();
        assert_eq!(article.number, "1");

        // Get article for 2025 - should get v2 article
        let article = resolver
            .get_article_by_output("test_law", "test_output", Some(date_2025))
            .unwrap();
        assert_eq!(article.number, "2");
    }

    #[test]
    fn test_resolver_mixed_valid_from() {
        // Test mixing laws with and without valid_from
        let mut resolver = RuleResolver::new();

        // Load version without valid_from
        resolver.load_from_yaml(make_test_law()).unwrap();

        // Load version with valid_from
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();

        assert_eq!(resolver.version_count(), 2);

        // The version with valid_from should be sorted first (has a date)
        // Version without valid_from should match any date
        let date_2024 = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let law = resolver.get_law_for_date("test_law", Some(date_2024));
        assert!(law.is_some()); // The None valid_from version should match
    }

    // -------------------------------------------------------------------------
    // New Method Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolver_output_count() {
        let mut resolver = RuleResolver::new();
        assert_eq!(resolver.output_count(), 0);

        resolver.load_from_yaml(make_test_law()).unwrap();
        assert_eq!(resolver.output_count(), 1);
    }

    #[test]
    fn test_resolver_list_all_outputs() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_test_law()).unwrap();

        let outputs = resolver.list_all_outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs.contains(&("test_law", "test_output")));
    }

    // -------------------------------------------------------------------------
    // Implements Index (IoC) Tests
    // -------------------------------------------------------------------------

    fn make_law_with_open_term() -> &'static str {
        r#"
$id: zorgtoeslagwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: De standaardpremie wordt vastgesteld bij ministeriele regeling
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 0
"#
    }

    fn make_implementing_regulation() -> &'static str {
        r#"
$id: regeling_standaardpremie
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 1928
    machine_readable:
      implements:
        - law: zorgtoeslagwet
          article: '4'
          open_term: standaardpremie
          gelet_op: "Gelet op artikel 4 van de Wet op de zorgtoeslag"
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 1928
"#
    }

    fn make_implementing_regulation_older() -> &'static str {
        r#"
$id: regeling_standaardpremie_2024
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 1889
    machine_readable:
      implements:
        - law: zorgtoeslagwet
          article: '4'
          open_term: standaardpremie
          gelet_op: "Gelet op artikel 4 van de Wet op de zorgtoeslag"
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 1889
"#
    }

    #[test]
    fn test_implements_index_populated() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();

        // Index should be populated
        assert_eq!(resolver.implements_count(), 1);

        // Look up
        let results = resolver
            .find_implementations(
                "zorgtoeslagwet",
                "4",
                "standaardpremie",
                None,
                &HashMap::new(),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "regeling_standaardpremie");
        assert_eq!(results[0].1.number, "1");
    }

    #[test]
    fn test_implements_index_no_match() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        // No implementing regulation loaded

        let results = resolver
            .find_implementations(
                "zorgtoeslagwet",
                "4",
                "standaardpremie",
                None,
                &HashMap::new(),
            )
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_implements_index_priority_lex_posterior() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation_older())
            .unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();

        assert_eq!(resolver.implements_count(), 2);

        let results = resolver
            .find_implementations(
                "zorgtoeslagwet",
                "4",
                "standaardpremie",
                None,
                &HashMap::new(),
            )
            .unwrap();
        assert_eq!(results.len(), 2);
        // Winner (newest) should be first
        assert_eq!(results[0].0.id, "regeling_standaardpremie");
        assert_eq!(results[1].0.id, "regeling_standaardpremie_2024");
    }

    #[test]
    fn test_implements_index_unload() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();

        assert_eq!(resolver.implements_count(), 1);

        resolver.unload_law("regeling_standaardpremie");
        assert_eq!(resolver.implements_count(), 0);

        let results = resolver
            .find_implementations(
                "zorgtoeslagwet",
                "4",
                "standaardpremie",
                None,
                &HashMap::new(),
            )
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_implements_index_backward_compat() {
        // Laws without implements should still load fine
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_test_law()).unwrap();

        assert_eq!(resolver.implements_count(), 0);
        assert_eq!(resolver.law_count(), 1);
    }

    fn get_regulation_path() -> std::path::PathBuf {
        std::env::var("REGULATION_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("corpus")
                    .join("regulation")
            })
    }

    #[test]
    fn test_resolver_load_from_directory() {
        let regulation_path = get_regulation_path().join("nl");

        let mut resolver = RuleResolver::new();
        let count = resolver.load_from_directory(&regulation_path).unwrap();

        assert!(
            count >= 10,
            "Expected at least 10 laws from corpus/regulation/nl, got {}",
            count
        );
        assert!(resolver.has_law("zorgtoeslagwet"));
        assert!(resolver.has_law("regeling_standaardpremie"));
        assert!(resolver.has_law("participatiewet"));
    }
}
