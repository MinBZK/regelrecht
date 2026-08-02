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

use crate::article::{
    Article, ArticleBasedLaw, HookFilter, HookPoint, LawLoad, ProcedureDefinition,
};
use crate::config;
use crate::error::{EngineError, Result};
use crate::priority::{self, Candidate};
use crate::types::Value;
use chrono::NaiveDate;
use regelrecht_shared::RegulatoryLayer;
use std::collections::HashMap;

/// Why a law version could not be selected for a reference date.
///
/// Used for honest diagnostics (RFC-019 §3): the engine states the *data fact*,
/// never a legal verdict like "geen grondslag" — eerbiedigende werking, a
/// statische verwijzing, or an alternative grondslag may keep the law alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionReason {
    /// No law with this id is loaded.
    NotFound,
    /// The law exists, but no version is in force yet on the reference date
    /// (every version has `valid_from` after it).
    NotYetInForce,
    /// The most recent applicable version ended before the reference date.
    /// Carries the `valid_to` date that was last in force.
    EndedOn(NaiveDate),
}

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
/// let article = resolver.get_article_by_output("wet_op_de_zorgtoeslag", "standaardpremie", None);
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

        // RFC-019: valid_to is static version-selection metadata. An unparseable
        // value (e.g. the format-valid but calendar-invalid '2023-02-30') would
        // silently skip the expiry check in select_in and keep an ended law in
        // force forever - reject it at load time instead. valid_from keeps its
        // lenient behaviour (it may be a '#'-reference per RFC-001).
        if let Some(valid_to) = &law.valid_to {
            parse_date(valid_to).map_err(|_| {
                EngineError::LoadError(format!(
                    "law '{law_id}': valid_to '{valid_to}' is not a valid date (expected YYYY-MM-DD)"
                ))
            })?;
        }

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

    /// Like [`Self::get_law_for_date`], but reports *why* selection failed.
    ///
    /// On failure returns a [`SelectionReason`] so callers can produce an honest
    /// diagnostic (RFC-019 §3) without inventing a legal verdict. A required
    /// `reference_date` is taken: the "no date → latest version" shortcut of
    /// [`Self::get_law_for_date`] does not apply here.
    pub fn get_law_for_date_result(
        &self,
        law_id: &str,
        reference_date: NaiveDate,
    ) -> std::result::Result<&ArticleBasedLaw, SelectionReason> {
        let versions = self
            .law_versions
            .get(law_id)
            .ok_or(SelectionReason::NotFound)?;
        Self::select_in(versions, reference_date)
    }

    /// Version-aware lookup reporting *why* selection failed, tolerating an
    /// absent reference date.
    ///
    /// With `Some(date)` this is [`Self::get_law_for_date_result`]; with `None`
    /// it keeps the "no date → latest version" behaviour of
    /// [`Self::get_law_for_date`], failing only when the law id is unknown.
    ///
    /// NOTE: `None` deliberately means "latest version regardless of validity
    /// window" - the `valid_to` upper bound is NOT applied, so an ended law's
    /// final version is returned. Execution paths always pass `Some(date)`
    /// (malformed calculation dates are rejected up front); only pass `None`
    /// for display/listing-style lookups. Revisit when RFC-020 threads
    /// `as_of` dates through every resolution.
    pub fn get_law_for_date_reported(
        &self,
        law_id: &str,
        reference_date: Option<NaiveDate>,
    ) -> std::result::Result<&ArticleBasedLaw, SelectionReason> {
        match reference_date {
            Some(date) => self.get_law_for_date_result(law_id, date),
            None => self
                .law_versions
                .get(law_id)
                .and_then(|versions| versions.first())
                .ok_or(SelectionReason::NotFound),
        }
    }

    /// Select the appropriate version for a reference date.
    ///
    /// # Selection Logic
    /// 1. Among versions with `valid_from <= reference_date`, take the most recent
    ///    (`valid_from`-less versions match any date). Versions are sorted newest-first.
    /// 2. If that version has a `valid_to` and `reference_date > valid_to`, the law is
    ///    no longer in force → return `None`. Do **not** fall through to an older
    ///    version: a repealed law does not resurrect a prior version (RFC-019 §2).
    fn select_version_for_date<'a>(
        &self,
        versions: &'a [ArticleBasedLaw],
        reference_date: NaiveDate,
    ) -> Option<&'a ArticleBasedLaw> {
        Self::select_in(versions, reference_date).ok()
    }

    /// Shared selection core over a version slice (sorted newest-first), reporting
    /// the reason on failure. See [`Self::select_version_for_date`] for the rule.
    fn select_in(
        versions: &[ArticleBasedLaw],
        reference_date: NaiveDate,
    ) -> std::result::Result<&ArticleBasedLaw, SelectionReason> {
        let candidate = versions
            .iter()
            .find(|v| {
                v.valid_from
                    .as_ref()
                    .and_then(|s| parse_date(s).ok())
                    .is_none_or(|valid_from| valid_from <= reference_date)
            })
            .ok_or(SelectionReason::NotYetInForce)?;

        // Upper bound (inclusive): in force iff reference_date <= valid_to.
        if let Some(valid_to) = candidate.valid_to.as_ref().and_then(|s| parse_date(s).ok()) {
            if reference_date > valid_to {
                return Err(SelectionReason::EndedOn(valid_to));
            }
        }
        Ok(candidate)
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
    /// (e.g., `gemeente_code`, `waterschap_code`). A law with no scope fields
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
        if let Some(ref law_provincie) = law.provincie_code {
            let scope_value = scope.get("provincie_code").and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            });
            match scope_value {
                Some(sp) if sp == law_provincie => {}
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

    /// The `delegation_type` an open term requires of its implementations.
    ///
    /// Returns `None` when the declaring law, article or term cannot be found,
    /// or when the term names no layer — all four mean the same thing here: the
    /// law demands nothing of the layer, so every layer may fill the term.
    fn required_delegation_type(
        &self,
        law_id: &str,
        article: &str,
        open_term_id: &str,
        reference_date: Option<NaiveDate>,
    ) -> Option<&str> {
        self.get_law_for_date(law_id, reference_date)?
            .find_article_by_number(article)?
            .get_open_terms()?
            .iter()
            .find(|t| t.id == open_term_id)?
            .delegation_type
            .as_deref()
    }

    /// Whether `law` may fill an open term that requires `delegation_type`.
    ///
    /// Delegation is a question of competence, not of precedence: an article
    /// that reserves a term for a ministeriële regeling has not authorised a
    /// beleidsregel to fill it, and a beleidsregel that does so anyway is not a
    /// weaker implementation but no implementation at all. So the gate sits
    /// *before* [`priority::resolve_candidate`], which ranks implementations
    /// that are all competent.
    ///
    /// A mismatching layer is rejected silently, the way
    /// [`Self::matches_scope`] rejects a regulation from another municipality:
    /// the candidate simply is not one, and a competent candidate lower in the
    /// ranking should still be able to win. Erroring here would let an
    /// unauthorised regulation take the whole resolution down with it, and it
    /// would also make "no implementation found" — which the caller already
    /// handles, via the open term's `default` or its `required` flag —
    /// unreachable for exactly the case where the law never granted anyone the
    /// power in the first place.
    ///
    /// A `delegation_type` that names no known regulatory layer is a different
    /// matter and is an error. Comparing an unknown string against every layer
    /// would silently reject every candidate, so the engine would answer "no
    /// implementation" to a question it did not understand. That is the same
    /// reason `compare_law_priority` errors on an unresolvable collision rather
    /// than guessing.
    fn matches_delegation(
        law: &ArticleBasedLaw,
        declaring_law_id: &str,
        open_term_id: &str,
        delegation_type: &str,
    ) -> Result<bool> {
        if RegulatoryLayer::from_yaml_str(delegation_type).is_none() {
            return Err(EngineError::ResolutionError(format!(
                "Open term '{open_term_id}' on {declaring_law_id} declares delegation_type \
                 '{delegation_type}', which is not a known regulatory layer"
            )));
        }
        Ok(law.regulatory_layer.as_str() == delegation_type)
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

        // What layer the declaring article demands of an implementation. Absent
        // means the law demands nothing, and then every layer may fill the term.
        let delegation_type =
            self.required_delegation_type(law_id, article, open_term_id, reference_date);

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

            // Delegation gate: a layer the declaring article did not authorise
            // is not a weaker candidate but no candidate at all.
            if let Some(expected) = delegation_type {
                if !Self::matches_delegation(law, law_id, open_term_id, expected)? {
                    tracing::debug!(
                        candidate = %entry.law_id,
                        candidate_layer = %law.regulatory_layer.as_str(),
                        delegation_type = %expected,
                        "Skipping: regulatory_layer is not the layer the open term delegates to"
                    );
                    continue;
                }
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

    fn make_test_law_with_validity(valid_from: &str, valid_to: &str, value: i32) -> String {
        format!(
            r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '{valid_from}'
valid_to: '{valid_to}'
articles:
  - number: '1'
    text: Test article {valid_from} t/m {valid_to}
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
    fn test_resolver_valid_to_upper_bound() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_validity(
                "2023-01-01",
                "2023-12-31",
                100,
            ))
            .unwrap();

        // In force before and on the (inclusive) end date.
        assert!(resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2023, 6, 1).unwrap())
            )
            .is_some());
        assert!(resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap())
            )
            .is_some());

        // No longer in force the day after valid_to.
        assert!(resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
            )
            .is_none());
    }

    #[test]
    fn test_resolver_valid_to_no_fall_through() {
        // An older open-ended version (100) and a newer version (200) that ends 2024-12-31.
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2023-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_validity(
                "2024-01-01",
                "2024-12-31",
                200,
            ))
            .unwrap();

        // Before the newer version: the older one applies.
        let law = resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2023, 6, 1).unwrap()),
            )
            .unwrap();
        assert_eq!(law.valid_from, Some("2023-01-01".to_string()));

        // After the newer version has ended: the law is no longer in force.
        // It must NOT resurrect the older open-ended version (RFC-019 §2).
        assert!(resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap())
            )
            .is_none());
    }

    #[test]
    fn test_resolver_selection_reason() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_validity(
                "2023-01-01",
                "2023-12-31",
                100,
            ))
            .unwrap();

        // Unknown law id.
        assert_eq!(
            resolver.get_law_for_date_result("nope", NaiveDate::from_ymd_opt(2023, 6, 1).unwrap()),
            Err(SelectionReason::NotFound)
        );
        // Before it is in force.
        assert_eq!(
            resolver
                .get_law_for_date_result("test_law", NaiveDate::from_ymd_opt(2022, 1, 1).unwrap()),
            Err(SelectionReason::NotYetInForce)
        );
        // After it has ended — states the data fact, not a verdict.
        assert_eq!(
            resolver
                .get_law_for_date_result("test_law", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            Err(SelectionReason::EndedOn(
                NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
            ))
        );
        // In force.
        assert!(resolver
            .get_law_for_date_result("test_law", NaiveDate::from_ymd_opt(2023, 6, 1).unwrap())
            .is_ok());
    }

    #[test]
    fn test_resolver_rejects_calendar_invalid_valid_to() {
        // Format-valid but calendar-invalid (passes the schema regex): without
        // the load-time check this would silently skip the expiry check and
        // keep the law in force forever (RFC-019).
        let mut resolver = RuleResolver::new();
        let result = resolver.load_from_yaml(&make_test_law_with_validity(
            "2023-01-01",
            "2023-02-30",
            100,
        ));
        assert!(matches!(result, Err(EngineError::LoadError(_))));
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
$id: wet_op_de_zorgtoeslag
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
        - law: wet_op_de_zorgtoeslag
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
        - law: wet_op_de_zorgtoeslag
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
                "wet_op_de_zorgtoeslag",
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
                "wet_op_de_zorgtoeslag",
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
                "wet_op_de_zorgtoeslag",
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

    // -------------------------------------------------------------------------
    // Delegation gate: who the law authorises to fill an open term
    // -------------------------------------------------------------------------

    /// A beleidsregel claiming an open term that the law reserves for a
    /// ministeriële regeling. Uitvoeringsbeleid is not an algemeen verbindend
    /// voorschrift, so it cannot fill a delegated term however recent it is.
    fn make_beleidsregel_claiming_standaardpremie() -> &'static str {
        r#"
$id: beleidsregel_standaardpremie
regulatory_layer: BELEIDSREGEL
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 2500
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: '4'
          open_term: standaardpremie
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 2500
"#
    }

    #[test]
    fn test_find_implementations_rejects_wrong_delegation_layer() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_beleidsregel_claiming_standaardpremie())
            .unwrap();

        // The declaration is indexed — the gate is about competence, not indexing.
        assert_eq!(resolver.implements_count(), 1);

        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                "standaardpremie",
                None,
                &HashMap::new(),
            )
            .unwrap();
        assert!(
            results.is_empty(),
            "a BELEIDSREGEL may not fill a term delegated to a MINISTERIELE_REGELING"
        );
    }

    /// The gate must not merely demote an unauthorised candidate: the
    /// ministeriële regeling wins even though the beleidsregel is newer, and
    /// the beleidsregel does not appear among the results at all.
    #[test]
    fn test_find_implementations_wrong_layer_does_not_shadow_authorised_one() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_beleidsregel_claiming_standaardpremie())
            .unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();

        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                "standaardpremie",
                None,
                &HashMap::new(),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "regeling_standaardpremie");
    }

    /// An open term without a `delegation_type` demands nothing of the layer,
    /// so uitvoeringsbeleid may fill it.
    #[test]
    fn test_find_implementations_without_delegation_type_accepts_any_layer() {
        let mut resolver = RuleResolver::new();

        resolver
            .load_from_yaml(
                r#"
$id: wet_zonder_delegatie
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan handelt redelijkerwijs
    machine_readable:
      open_terms:
        - id: redelijke_termijn_dagen
          type: number
          required: true
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: beleid_redelijke_termijn
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Een redelijke termijn is acht weken
    machine_readable:
      implements:
        - law: wet_zonder_delegatie
          article: '1'
          open_term: redelijke_termijn_dagen
      execution:
        output:
          - name: redelijke_termijn_dagen
            type: number
        actions:
          - output: redelijke_termijn_dagen
            value: 56
"#,
            )
            .unwrap();

        let results = resolver
            .find_implementations(
                "wet_zonder_delegatie",
                "1",
                "redelijke_termijn_dagen",
                None,
                &HashMap::new(),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "beleid_redelijke_termijn");
    }

    /// A `delegation_type` naming no known layer is an error, not a silent
    /// rejection of everything: the engine must not answer "no implementation"
    /// to a question it did not understand.
    #[test]
    fn test_find_implementations_unknown_delegation_type_is_error() {
        let mut resolver = RuleResolver::new();

        resolver
            .load_from_yaml(
                r#"
$id: wet_met_typo
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bij ministeriele regeling wordt het bedrag vastgesteld
    machine_readable:
      open_terms:
        - id: bedrag
          type: amount
          required: true
          delegation_type: MINISTERIELE_REGELIN
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: regeling_bedrag
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het bedrag bedraagt 100
    machine_readable:
      implements:
        - law: wet_met_typo
          article: '1'
          open_term: bedrag
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#,
            )
            .unwrap();

        let err = resolver
            .find_implementations("wet_met_typo", "1", "bedrag", None, &HashMap::new())
            .unwrap_err();
        assert!(
            matches!(err, EngineError::ResolutionError(ref m)
                if m.contains("MINISTERIELE_REGELIN") && m.contains("not a known regulatory layer")),
            "unexpected error: {err:?}"
        );
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
                "wet_op_de_zorgtoeslag",
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
        assert!(resolver.has_law("wet_op_de_zorgtoeslag"));
        assert!(resolver.has_law("regeling_standaardpremie"));
        assert!(resolver.has_law("participatiewet"));
    }

    // -------------------------------------------------------------------------
    // Law count limit (memory-exhaustion guard)
    // -------------------------------------------------------------------------

    /// A minimal law with its own `$id`, so the registry can be filled to the limit.
    fn make_numbered_law(n: usize, valid_from: Option<&str>) -> String {
        let valid_from_line = match valid_from {
            Some(d) => format!("valid_from: '{d}'\n"),
            None => String::new(),
        };
        format!(
            r#"
$id: filler_law_{n}
regulatory_layer: WET
publication_date: '2025-01-01'
{valid_from_line}articles:
  - number: '1'
    text: Filler article {n}
    machine_readable:
      execution:
        output:
          - name: filler_output
            type: number
        actions:
          - output: filler_output
            value: {n}
"#
        )
    }

    fn resolver_at_law_limit() -> RuleResolver {
        let mut resolver = RuleResolver::new();
        for n in 0..config::MAX_LOADED_LAWS {
            resolver
                .load_from_yaml(&make_numbered_law(n, None))
                .unwrap();
        }
        assert_eq!(resolver.version_count(), config::MAX_LOADED_LAWS);
        resolver
    }

    #[test]
    fn test_resolver_limit_rejects_new_version_of_existing_law() {
        // A second version of an already-loaded law is a *new* law in the
        // registry: it adds memory, so it must hit the limit like any other.
        let mut resolver = resolver_at_law_limit();

        let result = resolver.load_from_yaml(&make_numbered_law(0, Some("2030-01-01")));

        assert!(
            matches!(result, Err(EngineError::LoadError(_))),
            "adding a new version at the limit must be rejected, got {result:?}"
        );
        assert_eq!(resolver.version_count(), config::MAX_LOADED_LAWS);
        assert_eq!(resolver.version_count_for_law("filler_law_0"), 1);
    }

    #[test]
    fn test_resolver_limit_allows_replacing_existing_version() {
        // Replacing a version (same law id, same valid_from) does not grow the
        // registry, so the limit must not block it — otherwise a full resolver
        // could never be corrected.
        let mut resolver = resolver_at_law_limit();

        resolver
            .load_from_yaml(&make_numbered_law(0, None))
            .expect("replacing an existing version at the limit must be allowed");

        assert_eq!(resolver.version_count(), config::MAX_LOADED_LAWS);
    }

    #[test]
    fn test_resolver_law_count_counts_unique_ids() {
        let mut resolver = RuleResolver::new();
        assert_eq!(resolver.law_count(), 0);

        resolver
            .load_from_yaml(&make_numbered_law(1, None))
            .unwrap();
        resolver
            .load_from_yaml(&make_numbered_law(2, None))
            .unwrap();
        assert_eq!(resolver.law_count(), 2);

        // A second version of law 1 adds a version, not a law id.
        resolver
            .load_from_yaml(&make_numbered_law(1, Some("2030-01-01")))
            .unwrap();
        assert_eq!(resolver.law_count(), 2);
        assert_eq!(resolver.version_count(), 3);
    }

    #[test]
    fn test_resolver_all_law_versions_yields_every_version() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();
        resolver
            .load_from_yaml(&make_numbered_law(7, None))
            .unwrap();

        let mut seen: Vec<(&str, Option<&str>)> = resolver
            .all_law_versions()
            .map(|law| (law.id.as_str(), law.valid_from.as_deref()))
            .collect();
        seen.sort();

        assert_eq!(
            seen,
            vec![
                ("filler_law_7", None),
                ("test_law", Some("2024-01-01")),
                ("test_law", Some("2025-01-01")),
            ]
        );
    }

    // -------------------------------------------------------------------------
    // Version unloading
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolver_unload_version_removes_only_the_named_version() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();

        assert!(resolver.unload_law_version("test_law", Some("2024-01-01")));

        // The version that was *not* named must survive.
        assert_eq!(resolver.version_count(), 1);
        let remaining = resolver.get_law("test_law").unwrap();
        assert_eq!(remaining.valid_from, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_resolver_unload_unknown_version_is_a_no_op() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();

        // Nothing matches, so nothing was removed — the caller must be told so.
        assert!(!resolver.unload_law_version("test_law", Some("1999-01-01")));
        assert!(!resolver.unload_law_version("test_law", None));
        assert_eq!(resolver.version_count(), 1);
        assert!(resolver.has_law("test_law"));

        // Unknown law id likewise.
        assert!(!resolver.unload_law_version("nonexistent", Some("2024-01-01")));
    }

    // -------------------------------------------------------------------------
    // Scope filtering (gemeentelijke / waterschaps-verordeningen)
    // -------------------------------------------------------------------------

    fn make_law_with_open_term_scoped() -> &'static str {
        r#"
$id: kaderwet
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief wordt vastgesteld bij verordening
    machine_readable:
      open_terms:
        - id: tarief
          type: amount
          required: true
          delegated_to: waterschap
          delegation_type: WATERSCHAPS_VERORDENING
      execution:
        output:
          - name: tarief
            type: number
        actions:
          - output: tarief
            value: 0
"#
    }

    fn make_scoped_implementation(id: &str, scope_line: &str, value: i32) -> String {
        format!(
            r#"
$id: {id}
regulatory_layer: WATERSCHAPS_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
{scope_line}
articles:
  - number: '1'
    text: Het tarief bedraagt {value}
    machine_readable:
      implements:
        - law: kaderwet
          article: '1'
          open_term: tarief
          gelet_op: "Gelet op artikel 1 van de Kaderwet"
      execution:
        output:
          - name: tarief
            type: number
        actions:
          - output: tarief
            value: {value}
"#
        )
    }

    fn scope_of(field: &str, code: &str) -> HashMap<String, Value> {
        let mut scope = HashMap::new();
        scope.insert(field.to_string(), Value::String(code.to_string()));
        scope
    }

    /// A waterschapsverordening only implements an open term for its own
    /// waterschap; another waterschap (or a national run) must not inherit it.
    #[test]
    fn test_find_implementations_filters_on_waterschap_code() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(make_law_with_open_term_scoped())
            .unwrap();
        resolver
            .load_from_yaml(&make_scoped_implementation(
                "keur_ws0653",
                "waterschap_code: 'WS0653'",
                42,
            ))
            .unwrap();

        let find = |scope: &HashMap<String, Value>| {
            resolver
                .find_implementations("kaderwet", "1", "tarief", None, scope)
                .unwrap()
        };

        // Own waterschap: applies.
        let matching = find(&scope_of("waterschap_code", "WS0653"));
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].0.id, "keur_ws0653");

        // Different waterschap: does not apply.
        assert!(find(&scope_of("waterschap_code", "WS0999")).is_empty());

        // No waterschap in scope at all: does not apply.
        assert!(find(&HashMap::new()).is_empty());

        // A gemeente in scope does not satisfy a waterschap requirement.
        assert!(find(&scope_of("gemeente_code", "WS0653")).is_empty());
    }

    /// The gemeente branch of the same rule, so both scope fields are pinned.
    #[test]
    fn test_find_implementations_filters_on_gemeente_code() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(make_law_with_open_term_scoped())
            .unwrap();
        resolver
            .load_from_yaml(&make_scoped_implementation(
                "verordening_g0363",
                "gemeente_code: 'GM0363'",
                7,
            ))
            .unwrap();

        let find = |scope: &HashMap<String, Value>| {
            resolver
                .find_implementations("kaderwet", "1", "tarief", None, scope)
                .unwrap()
        };

        assert_eq!(find(&scope_of("gemeente_code", "GM0363")).len(), 1);
        assert!(find(&scope_of("gemeente_code", "GM0599")).is_empty());
        assert!(find(&HashMap::new()).is_empty());
    }

    /// The provincie branch, which the comment on `matches_scope` already
    /// promised while the code did not have it: a provinciale verordening was
    /// national as far as the resolver was concerned.
    #[test]
    fn test_find_implementations_filters_on_provincie_code() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(
                r#"
$id: kaderwet_provinciaal
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief wordt vastgesteld bij provinciale verordening
    machine_readable:
      open_terms:
        - id: tarief
          type: amount
          required: true
          delegated_to: provinciale staten
          delegation_type: PROVINCIALE_VERORDENING
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: verordening_pv27
regulatory_layer: PROVINCIALE_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
provincie_code: PV27
articles:
  - number: '1'
    text: Het tarief bedraagt 12
    machine_readable:
      implements:
        - law: kaderwet_provinciaal
          article: '1'
          open_term: tarief
      execution:
        output:
          - name: tarief
            type: number
        actions:
          - output: tarief
            value: 12
"#,
            )
            .unwrap();

        let find = |scope: &HashMap<String, Value>| {
            resolver
                .find_implementations("kaderwet_provinciaal", "1", "tarief", None, scope)
                .unwrap()
        };

        assert_eq!(find(&scope_of("provincie_code", "PV27")).len(), 1);
        assert!(find(&scope_of("provincie_code", "PV26")).is_empty());
        assert!(find(&HashMap::new()).is_empty());
        // A gemeente in scope does not satisfy a provincie requirement.
        assert!(find(&scope_of("gemeente_code", "PV27")).is_empty());
    }

    // -------------------------------------------------------------------------
    // Index bookkeeping: hooks, overrides, procedures
    // -------------------------------------------------------------------------

    fn make_law_with_hook(id: &str, article: &str, decision_type: Option<&str>) -> String {
        let decision_type_line = match decision_type {
            Some(dt) => format!("            decision_type: {dt}\n"),
            None => String::new(),
        };
        format!(
            r#"
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '{article}'
    text: Hook article
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
{decision_type_line}      execution:
        output:
          - name: hook_output_{id}
            type: number
        actions:
          - output: hook_output_{id}
            value: 1
"#
        )
    }

    fn make_law_with_override(id: &str, output: &str) -> String {
        format!(
            r#"
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '69'
    text: In afwijking van artikel 1 van de doelwet
    machine_readable:
      overrides:
        - law: doelwet
          article: '1'
          output: {output}
      execution:
        output:
          - name: {output}
            type: number
        actions:
          - output: {output}
            value: 4
"#
        )
    }

    fn make_override_target() -> &'static str {
        r#"
$id: doelwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De termijn bedraagt zes weken
    machine_readable:
      execution:
        output:
          - name: bezwaartermijn_weken
            type: number
        actions:
          - output: bezwaartermijn_weken
            value: 6
"#
    }

    fn make_law_with_procedure(id: &str, legal_character: &str, proc_id: &str) -> String {
        format!(
            r#"
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
procedure:
  - id: {proc_id}
    default: true
    applies_to:
      legal_character: {legal_character}
    stages:
      - name: AANVRAAG
        description: Belanghebbende dient aanvraag in
      - name: BESLUIT
        description: Bestuursorgaan neemt besluit
articles:
  - number: '1'
    text: Procedure-artikel
    machine_readable:
      execution:
        output:
          - name: proc_output_{proc_id}
            type: number
        actions:
          - output: proc_output_{proc_id}
            value: 1
"#
        )
    }

    /// Loading an unrelated law rebuilds only *its own* index entries. It must
    /// not evict the hooks, overrides and procedures other laws registered.
    #[test]
    fn test_loading_a_law_keeps_other_laws_indexes() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet",
                "bezwaartermijn_weken",
            ))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet", "3:46", None))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_procedure(
                "procedurewet",
                "BESCHIKKING",
                "beschikking",
            ))
            .unwrap();

        // Now load something entirely unrelated.
        resolver.load_from_yaml(make_test_law()).unwrap();

        let overrides = resolver.find_overrides("doelwet", "1", "bezwaartermijn_weken");
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].law_id, "afwijkingswet");

        let hooks = resolver.find_hooks(HookPoint::PostActions, "BESCHIKKING", None, "BESLUIT");
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].law_id, "hookwet");

        assert!(resolver.find_procedure("BESCHIKKING", None).is_some());
        assert!(resolver
            .find_procedure("BESCHIKKING", Some("beschikking"))
            .is_some());
    }

    /// Unloading one law must remove exactly that law's index entries and leave
    /// every other law's entries intact.
    #[test]
    fn test_unloading_a_law_leaves_other_laws_indexes_intact() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet_a",
                "bezwaartermijn_weken",
            ))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet_b",
                "bezwaartermijn_weken",
            ))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet_a", "3:46", None))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet_b", "3:47", None))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_procedure(
                "procedurewet_a",
                "BESCHIKKING",
                "beschikking",
            ))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_procedure(
                "procedurewet_b",
                "ALGEMEEN_VERBINDEND_VOORSCHRIFT",
                "avv",
            ))
            .unwrap();

        assert!(resolver.unload_law("afwijkingswet_a"));
        assert!(resolver.unload_law("hookwet_a"));
        assert!(resolver.unload_law("procedurewet_a"));

        // Overrides: only b survives, and it is really b.
        let overrides = resolver.find_overrides("doelwet", "1", "bezwaartermijn_weken");
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].law_id, "afwijkingswet_b");

        // Hooks: only b survives, and it is really b.
        let hooks = resolver.find_hooks(HookPoint::PostActions, "BESCHIKKING", None, "BESLUIT");
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].law_id, "hookwet_b");

        // Procedures: the unloaded law's procedure is gone, the other remains.
        assert!(resolver.find_procedure("BESCHIKKING", None).is_none());
        assert!(resolver
            .find_procedure("ALGEMEEN_VERBINDEND_VOORSCHRIFT", None)
            .is_some());
    }

    /// The output index must lose exactly the unloaded law's outputs.
    #[test]
    fn test_unloading_a_law_leaves_other_laws_outputs_indexed() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();
        resolver
            .load_from_yaml(&make_numbered_law(3, None))
            .unwrap();

        assert!(resolver.unload_law("test_law"));

        assert_eq!(
            resolver.list_all_outputs(),
            vec![("filler_law_3", "filler_output")]
        );
        assert_eq!(resolver.output_count(), 1);
    }

    /// Unloading one implementing regulation must not take its siblings with it.
    #[test]
    fn test_unloading_one_implementor_keeps_the_others() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation_older())
            .unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();
        assert_eq!(resolver.implements_count(), 2);

        resolver.unload_law("regeling_standaardpremie_2024");

        assert_eq!(resolver.implements_count(), 1);
        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                "standaardpremie",
                None,
                &HashMap::new(),
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "regeling_standaardpremie");
    }

    // -------------------------------------------------------------------------
    // Hook filtering
    // -------------------------------------------------------------------------

    /// A hook narrowed to one decision type fires for that type only — never for
    /// another type, and never for a decision without a type.
    #[test]
    fn test_find_hooks_filters_on_decision_type() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet", "3:46", Some("TOEKENNING")))
            .unwrap();

        let find = |dt: Option<&str>| {
            resolver
                .find_hooks(HookPoint::PostActions, "BESCHIKKING", dt, "BESLUIT")
                .len()
        };

        assert_eq!(find(Some("TOEKENNING")), 1);
        assert_eq!(find(Some("AFWIJZING")), 0);
        assert_eq!(find(None), 0);
    }

    /// A hook without a decision-type filter fires regardless of decision type.
    #[test]
    fn test_find_hooks_without_decision_type_filter_matches_all() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet", "3:46", None))
            .unwrap();

        for dt in [Some("TOEKENNING"), Some("AFWIJZING"), None] {
            assert_eq!(
                resolver
                    .find_hooks(HookPoint::PostActions, "BESCHIKKING", dt, "BESLUIT")
                    .len(),
                1,
                "unfiltered hook should fire for decision_type {dt:?}"
            );
        }

        // Stage still filters.
        assert_eq!(
            resolver
                .find_hooks(HookPoint::PostActions, "BESCHIKKING", None, "BEKENDMAKING")
                .len(),
            0
        );
    }

    // -------------------------------------------------------------------------
    // Procedure lookup
    // -------------------------------------------------------------------------

    #[test]
    fn test_find_procedure_by_default_and_by_id() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_law_with_procedure(
                "procedurewet",
                "BESCHIKKING",
                "beschikking",
            ))
            .unwrap();

        let by_default = resolver.find_procedure("BESCHIKKING", None).unwrap();
        assert_eq!(by_default.id, "beschikking");
        assert_eq!(by_default.stages.len(), 2);

        let by_id = resolver
            .find_procedure("BESCHIKKING", Some("beschikking"))
            .unwrap();
        assert_eq!(by_id.id, "beschikking");

        // Unknown procedure id, and unknown legal character.
        assert!(resolver
            .find_procedure("BESCHIKKING", Some("nope"))
            .is_none());
        assert!(resolver.find_procedure("ONBEKEND", None).is_none());
    }

    // -------------------------------------------------------------------------
    // Override target validation
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_override_targets_accepts_a_resolvable_override() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet",
                "bezwaartermijn_weken",
            ))
            .unwrap();

        assert_eq!(resolver.validate_override_targets(), Vec::<String>::new());
    }

    #[test]
    fn test_validate_override_targets_reports_missing_law() {
        let mut resolver = RuleResolver::new();
        // The target law 'doelwet' is never loaded.
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet",
                "bezwaartermijn_weken",
            ))
            .unwrap();

        let errors = resolver.validate_override_targets();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("non-existent law 'doelwet'"),
            "unexpected error: {}",
            errors[0]
        );
        assert!(errors[0].contains("afwijkingswet:69"));
    }

    #[test]
    fn test_validate_override_targets_reports_missing_article() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        // Overrides article '99', which 'doelwet' does not have.
        let yaml = r#"
$id: afwijkingswet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '69'
    text: In afwijking van artikel 99 van de doelwet
    machine_readable:
      overrides:
        - law: doelwet
          article: '99'
          output: bezwaartermijn_weken
      execution:
        output:
          - name: bezwaartermijn_weken
            type: number
        actions:
          - output: bezwaartermijn_weken
            value: 4
"#;
        resolver.load_from_yaml(yaml).unwrap();

        let errors = resolver.validate_override_targets();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("non-existent article '99'"),
            "unexpected error: {}",
            errors[0]
        );
    }

    #[test]
    fn test_validate_override_targets_reports_missing_output() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        // The article exists, but does not produce 'beroepstermijn_weken'.
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet",
                "beroepstermijn_weken",
            ))
            .unwrap();

        let errors = resolver.validate_override_targets();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("non-existent output 'beroepstermijn_weken'"),
            "unexpected error: {}",
            errors[0]
        );
        assert!(errors[0].contains("doelwet:1"));
    }
}
