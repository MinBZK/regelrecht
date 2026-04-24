//! Data source registry for external data resolution
//!
//! Provides a registry for data sources that can be queried during law execution.
//! Data sources are queried in priority order (highest first) when resolving
//! values that aren't found in the law context.
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{DataSourceRegistry, DictDataSource, Value};
//! use std::collections::HashMap;
//!
//! // Create a registry and add a data source
//! let mut registry = DataSourceRegistry::new();
//! let mut data = BTreeMap::new();
//! data.insert("person_123".to_string(), {
//!     let mut record = BTreeMap::new();
//!     record.insert("income".to_string(), Value::Int(50000));
//!     record.insert("age".to_string(), Value::Int(35));
//!     record
//! });
//!
//! let source = DictDataSource::new("persons", 10, data);
//! registry.add_source(Box::new(source));
//!
//! // Query the registry
//! let mut criteria = BTreeMap::new();
//! criteria.insert("BSN".to_string(), Value::String("123".to_string()));
//!
//! if let Some(match_result) = registry.resolve("income", &criteria) {
//!     println!("Found income: {} from {}", match_result.value, match_result.source_name);
//! }
//! ```

use crate::types::Value;
use std::collections::{BTreeMap, HashSet};

/// Result of a successful data source query.
#[derive(Debug, Clone)]
pub struct DataSourceMatch {
    /// The resolved value
    pub value: Value,
    /// Name of the data source that provided the value
    pub source_name: String,
    /// Type of the data source (e.g., "dict", "database")
    pub source_type: String,
}

/// A YAML-declared `select_on` filter criterion as understood by data sources.
///
/// `field` is the column name to filter on, `value` is the resolved literal
/// value (after `$variable` substitution by the resolution layer).
#[derive(Debug, Clone)]
pub struct SelectOn {
    pub field: String,
    pub value: Value,
}

/// Trait for data source implementations.
///
/// Data sources provide external data that can be queried during law execution.
/// Each source has a priority (higher = checked first) and can provide values
/// for specific fields.
pub trait DataSource: Send + Sync {
    /// Get the name of this data source.
    fn name(&self) -> &str;

    /// Get the priority of this data source (higher = checked first).
    fn priority(&self) -> i32;

    /// Get the type identifier for this data source (e.g., "dict", "database").
    fn source_type(&self) -> &str;

    /// Check if this data source can provide a value for the given field.
    ///
    /// This is a quick check that doesn't require the full lookup criteria.
    fn has_field(&self, field: &str) -> bool;

    /// Get a value from this data source.
    ///
    /// # Arguments
    /// * `field` - The field name to retrieve
    /// * `criteria` - Criteria for selecting the record (e.g., BSN, year)
    ///
    /// # Returns
    /// The value if found, or None if no matching record exists.
    fn get(&self, field: &str, criteria: &BTreeMap<String, Value>) -> Option<Value>;

    /// Get all available fields in this data source.
    fn fields(&self) -> Vec<&str>;

    /// Native query using YAML-declared metadata.
    ///
    /// This is the rich-query path that the engine uses when an input has
    /// `source.{table, field, fields, select_on}` set in YAML. The default
    /// implementation falls back to `get(field, criteria)` (ignoring
    /// `fields`, `select_on`, `as_array`) for sources that don't natively
    /// understand the richer model.
    ///
    /// # Arguments
    /// * `field` - Optional column name to project as the result. Used for
    ///   scalar inputs.
    /// * `fields` - Optional column projection list. Used for object inputs:
    ///   the result is a `Value::Object` with these columns.
    /// * `select_on` - Filter criteria from YAML; values have already been
    ///   resolved against the law parameters by the caller.
    /// * `criteria` - The current evaluation parameters; passed for sources
    ///   that need them as fallback (e.g. simple key-field lookups).
    /// * `as_array` - When true, return the entire matched record set as
    ///   `Value::Array(Object)` for FOREACH iteration. `field`/`fields`
    ///   control per-record projection.
    fn query_native(
        &self,
        field: Option<&str>,
        fields: Option<&[String]>,
        select_on: &[SelectOn],
        criteria: &BTreeMap<String, Value>,
        as_array: bool,
    ) -> Option<Value> {
        // Default impl ignores the rich metadata and falls back to get().
        let _ = (fields, select_on, as_array);
        if let Some(f) = field {
            self.get(f, criteria)
        } else {
            None
        }
    }
}

/// Dictionary-based data source with key-based lookup.
///
/// Stores data as nested HashMaps: record_key -> field_name -> value.
/// Records are looked up by building a key from the lookup criteria.
///
/// # Key Building
///
/// The record key is built from sorted criteria values joined by underscore:
/// - `{BSN: "123", year: 2025}` -> key "123_2025"
/// - `{gemeente_code: "0363"}` -> key "0363"
///
/// # Case Sensitivity
///
/// Field names are matched case-insensitively to handle variations
/// in how fields are referenced in laws.
#[derive(Debug, Clone)]
pub struct DictDataSource {
    name: String,
    priority: i32,
    /// Data: record_key -> field_name (lowercase) -> value
    data: BTreeMap<String, BTreeMap<String, Value>>,
    /// Index of all available field names (lowercase)
    field_index: HashSet<String>,
    /// When set, `get()` filters criteria to only these fields before building the
    /// lookup key. This is needed for `from_records()`, which stores records by a
    /// single key field, while `get()` would otherwise build a key from ALL criteria.
    key_fields: Option<Vec<String>>,
}

impl DictDataSource {
    /// Create a new dictionary data source.
    ///
    /// # Arguments
    /// * `name` - Name identifier for this data source
    /// * `priority` - Priority for resolution order (higher = checked first)
    /// * `data` - Data as record_key -> field_name -> value
    pub fn new(
        name: impl Into<String>,
        priority: i32,
        data: BTreeMap<String, BTreeMap<String, Value>>,
    ) -> Self {
        // Build field index with lowercase field names
        let field_index = data
            .values()
            .flat_map(|record| record.keys())
            .map(|k| k.to_lowercase())
            .collect();

        // Normalize data keys to lowercase
        let normalized_data = data
            .into_iter()
            .map(|(key, fields)| {
                let normalized_fields = fields
                    .into_iter()
                    .map(|(k, v)| (k.to_lowercase(), v))
                    .collect();
                (key, normalized_fields)
            })
            .collect();

        Self {
            name: name.into(),
            priority,
            data: normalized_data,
            field_index,
            key_fields: None,
        }
    }

    /// Create a dictionary data source from a flat list of records.
    ///
    /// # Arguments
    /// * `name` - Name identifier for this data source
    /// * `priority` - Priority for resolution order
    /// * `key_field` - Field name to use as the record key (case-insensitive)
    /// * `records` - List of records as field -> value maps
    ///
    /// # Returns
    /// The data source, or None if key_field is not found in any record.
    pub fn from_records(
        name: impl Into<String>,
        priority: i32,
        key_field: &str,
        records: Vec<BTreeMap<String, Value>>,
    ) -> Option<Self> {
        let key_field_lower = key_field.to_lowercase();
        let has_records = !records.is_empty();
        let mut data = BTreeMap::new();

        for record in records {
            // Find the key field (case-insensitive)
            let key_value = record
                .iter()
                .find(|(k, _)| k.to_lowercase() == key_field_lower)
                .map(|(_, v)| v.clone());

            if let Some(key_val) = key_value {
                let key = value_to_key(&key_val);
                data.insert(key, record);
            }
        }

        // If records were provided but none contained the key field, return None
        // to signal a configuration error (wrong key_field name).
        if data.is_empty() && has_records {
            return None;
        }

        let mut source = Self::new(name, priority, data);
        source.key_fields = Some(vec![key_field_lower]);
        Some(source)
    }

    /// Store a record in the data source.
    ///
    /// # Arguments
    /// * `key` - The record key
    /// * `fields` - Field values for this record
    pub fn store(&mut self, key: impl Into<String>, fields: BTreeMap<String, Value>) {
        let key = key.into();

        // Update field index
        for field_name in fields.keys() {
            self.field_index.insert(field_name.to_lowercase());
        }

        // Normalize field names to lowercase
        let normalized_fields = fields
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect();

        self.data.insert(key, normalized_fields);
    }

    /// Get the number of records in this data source.
    pub fn record_count(&self) -> usize {
        self.data.len()
    }
}

impl DataSource for DictDataSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn source_type(&self) -> &str {
        "dict"
    }

    fn has_field(&self, field: &str) -> bool {
        self.field_index.contains(&field.to_lowercase())
    }

    fn get(&self, field: &str, criteria: &BTreeMap<String, Value>) -> Option<Value> {
        // When key_fields is set (e.g. from_records), filter criteria to only
        // the key fields before building the lookup key. Otherwise a caller
        // passing extra criteria would produce a key that doesn't match any record.
        let key = match &self.key_fields {
            Some(fields) => {
                let filtered: BTreeMap<String, Value> = criteria
                    .iter()
                    .filter(|(k, _)| fields.contains(&k.to_lowercase()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                build_lookup_key(&filtered)
            }
            None => build_lookup_key(criteria),
        };

        // Look up record
        let record = self.data.get(&key)?;

        // Get field value (case-insensitive)
        record.get(&field.to_lowercase()).cloned()
    }

    fn fields(&self) -> Vec<&str> {
        self.field_index.iter().map(|s| s.as_str()).collect()
    }
}

/// Record-set data source with multi-criteria filtering, field aliases, and
/// optional array-of-records output.
///
/// Unlike `DictDataSource` which collapses records into a single key→record map,
/// `RecordSetDataSource` keeps the full record list and filters at query time.
/// This supports the orchestration patterns the Python wrapper used to do:
///
/// - **Multi-criteria filtering**: filter records by multiple criteria fields
///   (e.g. both `bsn` and `year`) before reading the requested field.
/// - **Field aliases**: expose a record column under a different input name
///   (e.g. column `mate_van_gevaar` is exposed as `advies_mate_van_gevaar`).
/// - **Array fields**: a synthetic input name that returns the entire matched
///   list of records as `Value::Array(Object)`, ready for FOREACH iteration.
#[derive(Debug, Clone)]
pub struct RecordSetDataSource {
    name: String,
    priority: i32,
    /// Records (each is a column → value map). Column names are normalized
    /// to lowercase at construction time.
    records: Vec<BTreeMap<String, Value>>,
    /// Index of all column names available in the records (lowercase).
    field_index: HashSet<String>,
    /// Single key field for fast lookup. When set, `get()` filters records
    /// where this column equals the value of the same-named criterion.
    key_field: Option<String>,
    /// Multi-criteria filter fields (lowercase). When set, `get()` filters
    /// records where each of these columns equals the value of the same-named
    /// criterion. Mutually exclusive with `key_field` semantically (both can
    /// be set but `select_on` takes precedence).
    select_on: Vec<String>,
    /// Field aliases: input_name (lowercase) → record column (lowercase).
    /// When `get()` is called with an aliased input name, the column is read
    /// from the matched record under the underlying name.
    aliases: BTreeMap<String, String>,
    /// Array field configuration: when set, requests for `array_field.0`
    /// (the input name) return the entire list of matched records as
    /// `Value::Array(Object)`. The optional projection (`array_field.1`)
    /// limits which columns are included in each object; an empty projection
    /// means "include the whole record".
    array_field: Option<(String, Vec<String>)>,
}

impl RecordSetDataSource {
    /// Start building a record-set data source.
    pub fn builder(name: impl Into<String>, priority: i32) -> RecordSetDataSourceBuilder {
        RecordSetDataSourceBuilder {
            name: name.into(),
            priority,
            records: None,
            key_field: None,
            select_on: Vec::new(),
            aliases: BTreeMap::new(),
            array_field: None,
        }
    }

    /// Number of records in this source.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Build a list of indices of records that match `criteria`.
    fn matching_indices(&self, criteria: &BTreeMap<String, Value>) -> Vec<usize> {
        // Determine which criterion fields to use as filters.
        let filter_fields: Vec<String> = if !self.select_on.is_empty() {
            self.select_on.clone()
        } else if let Some(kf) = &self.key_field {
            vec![kf.clone()]
        } else {
            // No filter fields configured: every record matches (callers will
            // typically take the first one).
            return (0..self.records.len()).collect();
        };

        // Lowercase criterion keys for comparison.
        let lc_criteria: BTreeMap<String, &Value> = criteria
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect();

        let mut indices = Vec::new();
        for (idx, record) in self.records.iter().enumerate() {
            let mut all_match = true;
            for field in &filter_fields {
                let crit_val = match lc_criteria.get(field) {
                    Some(v) => *v,
                    None => {
                        // Criterion not provided — record cannot match.
                        all_match = false;
                        break;
                    }
                };
                let rec_val = match record.get(field) {
                    Some(v) => v,
                    None => {
                        all_match = false;
                        break;
                    }
                };
                if !values_match(rec_val, crit_val) {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                indices.push(idx);
            }
        }
        indices
    }

    /// Project a record onto a list of fields. An empty list returns a clone
    /// of the entire record.
    fn project_record(record: &BTreeMap<String, Value>, fields: &[String]) -> Value {
        if fields.is_empty() {
            return Value::Object(record.clone());
        }
        let mut obj = BTreeMap::new();
        for f in fields {
            let lc = f.to_lowercase();
            if let Some(v) = record.get(&lc) {
                obj.insert(lc, v.clone());
            }
        }
        Value::Object(obj)
    }
}

impl DataSource for RecordSetDataSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn source_type(&self) -> &str {
        "record_set"
    }

    fn has_field(&self, field: &str) -> bool {
        let lc = field.to_lowercase();
        if self.field_index.contains(&lc) {
            return true;
        }
        if self.aliases.contains_key(&lc) {
            return true;
        }
        if let Some((array_input, _)) = &self.array_field {
            if array_input == &lc {
                return true;
            }
        }
        false
    }

    fn get(&self, field: &str, criteria: &BTreeMap<String, Value>) -> Option<Value> {
        let lc_field = field.to_lowercase();

        // Array-field input: return the full set of matched records as
        // an array of (optionally projected) objects.
        if let Some((array_input, projection)) = &self.array_field {
            if &lc_field == array_input {
                let indices = self.matching_indices(criteria);
                if indices.is_empty() {
                    return None;
                }
                let items: Vec<Value> = indices
                    .iter()
                    .map(|i| Self::project_record(&self.records[*i], projection))
                    .collect();
                return Some(Value::Array(items));
            }
        }

        // Field-alias input: rewrite to the underlying column name.
        let lookup_field = self.aliases.get(&lc_field).cloned().unwrap_or(lc_field);

        // Take the first matching record and read the requested column.
        let indices = self.matching_indices(criteria);
        for idx in indices {
            if let Some(v) = self.records[idx].get(&lookup_field) {
                return Some(v.clone());
            }
        }
        None
    }

    fn fields(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.field_index.iter().map(|s| s.as_str()).collect();
        for alias in self.aliases.keys() {
            out.push(alias.as_str());
        }
        if let Some((af, _)) = &self.array_field {
            out.push(af.as_str());
        }
        out
    }

    fn query_native(
        &self,
        field: Option<&str>,
        fields: Option<&[String]>,
        select_on: &[SelectOn],
        _criteria: &BTreeMap<String, Value>,
        as_array: bool,
    ) -> Option<Value> {
        // Find matching record indices using the YAML-supplied select_on
        // criteria. Each criterion's column must equal the criterion's value
        // (loose numeric/string compare via values_match).
        let mut indices: Vec<usize> = Vec::new();
        for (idx, record) in self.records.iter().enumerate() {
            let mut all_match = true;
            for c in select_on {
                let col = c.field.to_lowercase();
                let rec_val = match record.get(&col) {
                    Some(v) => v,
                    None => {
                        all_match = false;
                        break;
                    }
                };
                if !values_match(rec_val, &c.value) {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                indices.push(idx);
            }
        }

        if indices.is_empty() {
            return None;
        }

        // Array mode: return the whole list of matched records, projected.
        if as_array {
            let items: Vec<Value> = indices
                .iter()
                .map(|i| project_record_value(&self.records[*i], field, fields))
                .collect();
            return Some(Value::Array(items));
        }

        // Scalar/object mode: take the first matching record.
        let record = &self.records[indices[0]];
        Some(project_record_value(record, field, fields))
    }
}

/// Project a single record into a `Value` according to the requested field
/// or fields:
///
/// - If `fields` is set: build an object with those columns.
/// - Else if `field` is set: extract that single column (or `Null` if absent).
/// - Else: return the entire record as an object.
fn project_record_value(
    record: &BTreeMap<String, Value>,
    field: Option<&str>,
    fields: Option<&[String]>,
) -> Value {
    if let Some(field_list) = fields {
        let mut obj = BTreeMap::new();
        for f in field_list {
            let lc = f.to_lowercase();
            if let Some(v) = record.get(&lc) {
                obj.insert(lc, v.clone());
            }
        }
        return Value::Object(obj);
    }
    if let Some(f) = field {
        let lc = f.to_lowercase();
        return record.get(&lc).cloned().unwrap_or(Value::Null);
    }
    Value::Object(record.clone())
}

/// Builder for `RecordSetDataSource`.
pub struct RecordSetDataSourceBuilder {
    name: String,
    priority: i32,
    records: Option<Vec<BTreeMap<String, Value>>>,
    key_field: Option<String>,
    select_on: Vec<String>,
    aliases: BTreeMap<String, String>,
    array_field: Option<(String, Vec<String>)>,
}

impl RecordSetDataSourceBuilder {
    /// Provide the records to back this source. Required.
    pub fn records(mut self, records: Vec<BTreeMap<String, Value>>) -> Self {
        self.records = Some(records);
        self
    }

    /// Set a single key field for fast lookup. The criterion of the same name
    /// (case-insensitive) is used to filter records.
    pub fn key_field(mut self, field: impl Into<String>) -> Self {
        self.key_field = Some(field.into().to_lowercase());
        self
    }

    /// Set multiple criteria fields. Records are matched only when ALL of
    /// these fields equal the corresponding criteria.
    pub fn select_on(mut self, fields: Vec<String>) -> Self {
        self.select_on = fields.into_iter().map(|f| f.to_lowercase()).collect();
        self
    }

    /// Add a single field alias: requesting `input_name` returns the value of
    /// the `column_name` field from the matched record.
    pub fn alias(mut self, input_name: impl Into<String>, column_name: impl Into<String>) -> Self {
        self.aliases.insert(
            input_name.into().to_lowercase(),
            column_name.into().to_lowercase(),
        );
        self
    }

    /// Add many field aliases at once. See [`alias`].
    pub fn aliases(mut self, map: BTreeMap<String, String>) -> Self {
        for (k, v) in map {
            self.aliases.insert(k.to_lowercase(), v.to_lowercase());
        }
        self
    }

    /// Configure an array field: requesting `input_name` returns the entire
    /// list of matched records as `Value::Array(Object)`. The `projection`
    /// limits which columns are included in each object; pass an empty slice
    /// to include the whole record.
    pub fn array_field(mut self, input_name: impl Into<String>, projection: &[&str]) -> Self {
        let proj = projection.iter().map(|s| s.to_lowercase()).collect();
        self.array_field = Some((input_name.into().to_lowercase(), proj));
        self
    }

    /// Build the `RecordSetDataSource`.
    pub fn build(self) -> std::result::Result<RecordSetDataSource, String> {
        let records = self
            .records
            .ok_or_else(|| "RecordSetDataSource requires records".to_string())?;

        // Normalize record column names to lowercase.
        let normalized: Vec<BTreeMap<String, Value>> = records
            .into_iter()
            .map(|r| r.into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect())
            .collect();

        // Build field index from all observed columns.
        let field_index: HashSet<String> =
            normalized.iter().flat_map(|r| r.keys()).cloned().collect();

        Ok(RecordSetDataSource {
            name: self.name,
            priority: self.priority,
            records: normalized,
            field_index,
            key_field: self.key_field,
            select_on: self.select_on,
            aliases: self.aliases,
            array_field: self.array_field,
        })
    }
}

/// Compare two values for equality, treating numeric types loosely so that
/// `Int(2025)` matches `String("2025")` and `Float(35.0)` matches `Int(35)`.
/// This mirrors the Python pre-resolution logic which compared stringified
/// values across DataFrame columns.
fn values_match(a: &Value, b: &Value) -> bool {
    if a == b {
        return true;
    }
    // Numeric / string cross-comparison via string representation.
    let sa = value_to_key(a);
    let sb = value_to_key(b);
    sa == sb
}

/// Registry for data sources with priority-based resolution.
///
/// When resolving a value, data sources are queried in priority order
/// (highest priority first). The first source that provides a value wins.
#[derive(Default)]
pub struct DataSourceRegistry {
    /// Data sources, sorted by priority (highest first)
    sources: Vec<Box<dyn DataSource>>,
}

impl DataSourceRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Add a data source to the registry.
    ///
    /// Sources are automatically sorted by priority (highest first).
    pub fn add_source(&mut self, source: Box<dyn DataSource>) {
        self.sources.push(source);
        // Sort by priority descending
        self.sources
            .sort_by_key(|b| std::cmp::Reverse(b.priority()));
    }

    /// Remove a data source by name.
    ///
    /// # Returns
    /// `true` if a source was removed, `false` if not found.
    pub fn remove_source(&mut self, name: &str) -> bool {
        let before = self.sources.len();
        self.sources.retain(|s| s.name() != name);
        self.sources.len() < before
    }

    /// Clear all data sources.
    pub fn clear(&mut self) {
        self.sources.clear();
    }

    /// Check if any data source can provide a value for the given field.
    pub fn has_field(&self, field: &str) -> bool {
        self.sources.iter().any(|s| s.has_field(field))
    }

    /// Resolve a value from the data sources.
    ///
    /// Sources are queried in priority order. The first source that
    /// provides a value for the field wins.
    ///
    /// # Arguments
    /// * `field` - The field name to resolve
    /// * `criteria` - Criteria for record lookup
    ///
    /// # Returns
    /// A `DataSourceMatch` if the value was found, None otherwise.
    pub fn resolve(
        &self,
        field: &str,
        criteria: &BTreeMap<String, Value>,
    ) -> Option<DataSourceMatch> {
        for source in &self.sources {
            if !source.has_field(field) {
                continue;
            }

            if let Some(value) = source.get(field, criteria) {
                return Some(DataSourceMatch {
                    value,
                    source_name: source.name().to_string(),
                    source_type: source.source_type().to_string(),
                });
            }
        }
        None
    }

    /// Find a registered data source by exact name (case-sensitive).
    pub fn find_source_by_name(&self, name: &str) -> Option<&dyn DataSource> {
        self.sources
            .iter()
            .find(|s| s.name() == name)
            .map(|s| s.as_ref())
    }

    /// Native query: look up a source by table name and run a rich query
    /// against it using YAML-declared metadata. Returns `None` when the
    /// table is not registered or no record matches.
    pub fn resolve_native(
        &self,
        table: &str,
        field: Option<&str>,
        fields: Option<&[String]>,
        select_on: &[SelectOn],
        criteria: &BTreeMap<String, Value>,
        as_array: bool,
    ) -> Option<DataSourceMatch> {
        let source = self.find_source_by_name(table)?;
        let value = source.query_native(field, fields, select_on, criteria, as_array)?;
        Some(DataSourceMatch {
            value,
            source_name: source.name().to_string(),
            source_type: source.source_type().to_string(),
        })
    }

    /// Get the number of registered data sources.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// List all registered source names.
    pub fn list_sources(&self) -> Vec<&str> {
        self.sources.iter().map(|s| s.name()).collect()
    }

    /// Get all available fields across all sources.
    pub fn all_fields(&self) -> HashSet<String> {
        self.sources
            .iter()
            .flat_map(|s| s.fields())
            .map(|f| f.to_string())
            .collect()
    }
}

// Allow Debug for DataSourceRegistry even though Box<dyn DataSource> doesn't implement Debug
impl std::fmt::Debug for DataSourceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceRegistry")
            .field("source_count", &self.sources.len())
            .field(
                "sources",
                &self
                    .sources
                    .iter()
                    .map(|s| format!("{}(priority={})", s.name(), s.priority()))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Build a lookup key from criteria values.
///
/// Sorts criteria by key name and joins values with underscore.
fn build_lookup_key(criteria: &BTreeMap<String, Value>) -> String {
    let mut pairs: Vec<_> = criteria
        .iter()
        .map(|(k, v)| (k.to_lowercase(), v))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    pairs
        .iter()
        .map(|(_, v)| value_to_key(v))
        .collect::<Vec<_>>()
        .join("_")
}

/// Convert a Value to a string key.
fn value_to_key(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(_) | Value::Object(_) => "complex".to_string(),
        Value::Untranslatable { .. } => "untranslatable".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_person_data() -> BTreeMap<String, BTreeMap<String, Value>> {
        let mut data = BTreeMap::new();

        let mut person1 = BTreeMap::new();
        person1.insert("income".to_string(), Value::Int(50000));
        person1.insert("age".to_string(), Value::Int(35));
        person1.insert("name".to_string(), Value::String("Jan".to_string()));
        data.insert("123".to_string(), person1);

        let mut person2 = BTreeMap::new();
        person2.insert("income".to_string(), Value::Int(40000));
        person2.insert("age".to_string(), Value::Int(28));
        person2.insert("name".to_string(), Value::String("Piet".to_string()));
        data.insert("456".to_string(), person2);

        data
    }

    // -------------------------------------------------------------------------
    // DictDataSource Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_dict_source_basic() {
        let source = DictDataSource::new("persons", 10, make_person_data());

        assert_eq!(source.name(), "persons");
        assert_eq!(source.priority(), 10);
        assert_eq!(source.source_type(), "dict");
        assert_eq!(source.record_count(), 2);
    }

    #[test]
    fn test_dict_source_has_field() {
        let source = DictDataSource::new("persons", 10, make_person_data());

        assert!(source.has_field("income"));
        assert!(source.has_field("INCOME")); // Case insensitive
        assert!(source.has_field("age"));
        assert!(source.has_field("name"));
        assert!(!source.has_field("nonexistent"));
    }

    #[test]
    fn test_dict_source_get() {
        let source = DictDataSource::new("persons", 10, make_person_data());

        let mut criteria = BTreeMap::new();
        criteria.insert("BSN".to_string(), Value::String("123".to_string()));

        let income = source.get("income", &criteria);
        assert_eq!(income, Some(Value::Int(50000)));

        let age = source.get("age", &criteria);
        assert_eq!(age, Some(Value::Int(35)));
    }

    #[test]
    fn test_dict_source_get_case_insensitive() {
        let source = DictDataSource::new("persons", 10, make_person_data());

        let mut criteria = BTreeMap::new();
        criteria.insert("BSN".to_string(), Value::String("123".to_string()));

        // Field name should be case-insensitive
        assert_eq!(source.get("income", &criteria), Some(Value::Int(50000)));
        assert_eq!(source.get("INCOME", &criteria), Some(Value::Int(50000)));
        assert_eq!(source.get("Income", &criteria), Some(Value::Int(50000)));
    }

    #[test]
    fn test_dict_source_get_not_found() {
        let source = DictDataSource::new("persons", 10, make_person_data());

        let mut criteria = BTreeMap::new();
        criteria.insert("BSN".to_string(), Value::String("999".to_string()));

        let result = source.get("income", &criteria);
        assert!(result.is_none());
    }

    #[test]
    fn test_dict_source_store() {
        let mut source = DictDataSource::new("persons", 10, BTreeMap::new());

        let mut fields = BTreeMap::new();
        fields.insert("income".to_string(), Value::Int(60000));
        fields.insert("age".to_string(), Value::Int(42));
        source.store("789", fields);

        assert_eq!(source.record_count(), 1);
        assert!(source.has_field("income"));

        let mut criteria = BTreeMap::new();
        criteria.insert("key".to_string(), Value::String("789".to_string()));
        assert_eq!(source.get("income", &criteria), Some(Value::Int(60000)));
    }

    #[test]
    fn test_dict_source_from_records_missing_key_field() {
        // Records exist but none contain the key field → should return None
        let records = vec![
            {
                let mut r = BTreeMap::new();
                r.insert("name".to_string(), Value::String("Jan".to_string()));
                r.insert("income".to_string(), Value::Int(50000));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("name".to_string(), Value::String("Piet".to_string()));
                r.insert("income".to_string(), Value::Int(40000));
                r
            },
        ];

        let result = DictDataSource::from_records("persons", 10, "BSN", records);
        assert!(
            result.is_none(),
            "Expected None when key field is missing from all records"
        );
    }

    #[test]
    fn test_dict_source_from_records_empty_vec() {
        // Empty records vec → should return Some (empty source)
        let result = DictDataSource::from_records("persons", 10, "BSN", vec![]);
        assert!(
            result.is_some(),
            "Expected Some for empty records vec (no records = no error)"
        );
        assert_eq!(result.unwrap().record_count(), 0);
    }

    #[test]
    fn test_dict_source_from_records() {
        let records = vec![
            {
                let mut r = BTreeMap::new();
                r.insert("BSN".to_string(), Value::String("123".to_string()));
                r.insert("income".to_string(), Value::Int(50000));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("BSN".to_string(), Value::String("456".to_string()));
                r.insert("income".to_string(), Value::Int(40000));
                r
            },
        ];

        let source = DictDataSource::from_records("persons", 10, "BSN", records).unwrap();
        assert_eq!(source.record_count(), 2);

        // Criteria must use the key_field name ("BSN"), not an arbitrary name
        let mut criteria = BTreeMap::new();
        criteria.insert("BSN".to_string(), Value::String("123".to_string()));
        assert_eq!(source.get("income", &criteria), Some(Value::Int(50000)));
    }

    // -------------------------------------------------------------------------
    // DataSourceRegistry Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_registry_basic() {
        let mut registry = DataSourceRegistry::new();
        assert_eq!(registry.source_count(), 0);

        registry.add_source(Box::new(DictDataSource::new(
            "persons",
            10,
            make_person_data(),
        )));
        assert_eq!(registry.source_count(), 1);
    }

    #[test]
    fn test_registry_resolve() {
        let mut registry = DataSourceRegistry::new();
        registry.add_source(Box::new(DictDataSource::new(
            "persons",
            10,
            make_person_data(),
        )));

        let mut criteria = BTreeMap::new();
        criteria.insert("BSN".to_string(), Value::String("123".to_string()));

        let result = registry.resolve("income", &criteria).unwrap();
        assert_eq!(result.value, Value::Int(50000));
        assert_eq!(result.source_name, "persons");
        assert_eq!(result.source_type, "dict");
    }

    #[test]
    fn test_registry_priority_order() {
        let mut registry = DataSourceRegistry::new();

        // Add low priority source first
        let mut low_data = BTreeMap::new();
        let mut low_record = BTreeMap::new();
        low_record.insert("value".to_string(), Value::Int(100));
        low_data.insert("key".to_string(), low_record);
        registry.add_source(Box::new(DictDataSource::new("low", 1, low_data)));

        // Add high priority source second
        let mut high_data = BTreeMap::new();
        let mut high_record = BTreeMap::new();
        high_record.insert("value".to_string(), Value::Int(200));
        high_data.insert("key".to_string(), high_record);
        registry.add_source(Box::new(DictDataSource::new("high", 10, high_data)));

        let mut criteria = BTreeMap::new();
        criteria.insert("k".to_string(), Value::String("key".to_string()));

        // High priority source should win
        let result = registry.resolve("value", &criteria).unwrap();
        assert_eq!(result.value, Value::Int(200));
        assert_eq!(result.source_name, "high");
    }

    #[test]
    fn test_registry_fallback() {
        let mut registry = DataSourceRegistry::new();

        // High priority source without the field
        let mut high_data = BTreeMap::new();
        let mut high_record = BTreeMap::new();
        high_record.insert("other".to_string(), Value::Int(999));
        high_data.insert("key".to_string(), high_record);
        registry.add_source(Box::new(DictDataSource::new("high", 10, high_data)));

        // Low priority source with the field
        let mut low_data = BTreeMap::new();
        let mut low_record = BTreeMap::new();
        low_record.insert("value".to_string(), Value::Int(100));
        low_data.insert("key".to_string(), low_record);
        registry.add_source(Box::new(DictDataSource::new("low", 1, low_data)));

        let mut criteria = BTreeMap::new();
        criteria.insert("k".to_string(), Value::String("key".to_string()));

        // Should fall back to low priority source
        let result = registry.resolve("value", &criteria).unwrap();
        assert_eq!(result.value, Value::Int(100));
        assert_eq!(result.source_name, "low");
    }

    #[test]
    fn test_registry_remove_source() {
        let mut registry = DataSourceRegistry::new();
        registry.add_source(Box::new(DictDataSource::new(
            "persons",
            10,
            make_person_data(),
        )));

        assert!(registry.remove_source("persons"));
        assert_eq!(registry.source_count(), 0);
        assert!(!registry.remove_source("nonexistent"));
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = DataSourceRegistry::new();
        registry.add_source(Box::new(DictDataSource::new("a", 1, BTreeMap::new())));
        registry.add_source(Box::new(DictDataSource::new("b", 2, BTreeMap::new())));

        registry.clear();
        assert_eq!(registry.source_count(), 0);
    }

    #[test]
    fn test_registry_has_field() {
        let mut registry = DataSourceRegistry::new();
        registry.add_source(Box::new(DictDataSource::new(
            "persons",
            10,
            make_person_data(),
        )));

        assert!(registry.has_field("income"));
        assert!(registry.has_field("age"));
        assert!(!registry.has_field("nonexistent"));
    }

    #[test]
    fn test_registry_list_sources() {
        let mut registry = DataSourceRegistry::new();
        registry.add_source(Box::new(DictDataSource::new("a", 5, BTreeMap::new())));
        registry.add_source(Box::new(DictDataSource::new("b", 10, BTreeMap::new())));
        registry.add_source(Box::new(DictDataSource::new("c", 1, BTreeMap::new())));

        let sources = registry.list_sources();
        // Should be sorted by priority (highest first)
        assert_eq!(sources, vec!["b", "a", "c"]);
    }

    #[test]
    fn test_registry_all_fields() {
        let mut registry = DataSourceRegistry::new();
        registry.add_source(Box::new(DictDataSource::new(
            "persons",
            10,
            make_person_data(),
        )));

        let fields = registry.all_fields();
        assert!(fields.contains("income"));
        assert!(fields.contains("age"));
        assert!(fields.contains("name"));
    }

    // -------------------------------------------------------------------------
    // Key Building Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_build_lookup_key_single() {
        let mut criteria = BTreeMap::new();
        criteria.insert("BSN".to_string(), Value::String("123".to_string()));

        let key = build_lookup_key(&criteria);
        assert_eq!(key, "123");
    }

    #[test]
    fn test_build_lookup_key_multiple() {
        let mut criteria = BTreeMap::new();
        criteria.insert("BSN".to_string(), Value::String("123".to_string()));
        criteria.insert("year".to_string(), Value::Int(2025));

        let key = build_lookup_key(&criteria);
        // Keys are sorted alphabetically
        assert_eq!(key, "123_2025");
    }

    #[test]
    fn test_build_lookup_key_case_insensitive_sort() {
        // Mixed-case keys should produce the same lookup key regardless of casing
        let mut criteria_upper = BTreeMap::new();
        criteria_upper.insert("BSN".to_string(), Value::String("123".to_string()));
        criteria_upper.insert("Year".to_string(), Value::Int(2025));

        let mut criteria_lower = BTreeMap::new();
        criteria_lower.insert("bsn".to_string(), Value::String("123".to_string()));
        criteria_lower.insert("year".to_string(), Value::Int(2025));

        assert_eq!(
            build_lookup_key(&criteria_upper),
            build_lookup_key(&criteria_lower),
            "Mixed-case keys should produce identical lookup keys"
        );
    }

    #[test]
    fn test_value_to_key() {
        assert_eq!(value_to_key(&Value::String("test".to_string())), "test");
        assert_eq!(value_to_key(&Value::Int(42)), "42");
        assert_eq!(value_to_key(&Value::Float(3.14)), "3.14");
        assert_eq!(value_to_key(&Value::Bool(true)), "true");
        assert_eq!(value_to_key(&Value::Null), "null");
    }

    // -------------------------------------------------------------------------
    // RecordSetDataSource Tests
    // -------------------------------------------------------------------------

    fn make_records() -> Vec<BTreeMap<String, Value>> {
        vec![
            {
                let mut r = BTreeMap::new();
                r.insert("bsn".to_string(), Value::String("123".to_string()));
                r.insert("year".to_string(), Value::Int(2024));
                r.insert("income".to_string(), Value::Int(50000));
                r.insert(
                    "mate_van_gevaar".to_string(),
                    Value::String("hoog".to_string()),
                );
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("bsn".to_string(), Value::String("123".to_string()));
                r.insert("year".to_string(), Value::Int(2025));
                r.insert("income".to_string(), Value::Int(55000));
                r.insert(
                    "mate_van_gevaar".to_string(),
                    Value::String("middel".to_string()),
                );
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("bsn".to_string(), Value::String("456".to_string()));
                r.insert("year".to_string(), Value::Int(2025));
                r.insert("income".to_string(), Value::Int(40000));
                r.insert(
                    "mate_van_gevaar".to_string(),
                    Value::String("laag".to_string()),
                );
                r
            },
        ]
    }

    #[test]
    fn test_record_set_simple_lookup() {
        let source = RecordSetDataSource::builder("persons", 10)
            .key_field("bsn")
            .records(make_records())
            .build()
            .unwrap();

        let mut criteria = BTreeMap::new();
        criteria.insert("bsn".to_string(), Value::String("456".to_string()));
        assert_eq!(source.get("income", &criteria), Some(Value::Int(40000)));
    }

    #[test]
    fn test_record_set_multi_criteria() {
        let source = RecordSetDataSource::builder("persons", 10)
            .select_on(vec!["bsn".to_string(), "year".to_string()])
            .records(make_records())
            .build()
            .unwrap();

        // BSN 123 has two records (2024, 2025) — multi-criteria narrows it down
        let mut criteria = BTreeMap::new();
        criteria.insert("bsn".to_string(), Value::String("123".to_string()));
        criteria.insert("year".to_string(), Value::Int(2024));
        assert_eq!(source.get("income", &criteria), Some(Value::Int(50000)));

        let mut criteria = BTreeMap::new();
        criteria.insert("bsn".to_string(), Value::String("123".to_string()));
        criteria.insert("year".to_string(), Value::Int(2025));
        assert_eq!(source.get("income", &criteria), Some(Value::Int(55000)));
    }

    #[test]
    fn test_record_set_field_alias() {
        let source = RecordSetDataSource::builder("persons", 10)
            .key_field("bsn")
            .alias("advies_mate_van_gevaar", "mate_van_gevaar")
            .records(make_records())
            .build()
            .unwrap();

        let mut criteria = BTreeMap::new();
        criteria.insert("bsn".to_string(), Value::String("456".to_string()));
        // Looked up via alias, returns the column value
        assert_eq!(
            source.get("advies_mate_van_gevaar", &criteria),
            Some(Value::String("laag".to_string()))
        );
        // Original field still works
        assert_eq!(
            source.get("mate_van_gevaar", &criteria),
            Some(Value::String("laag".to_string()))
        );
        assert!(source.has_field("advies_mate_van_gevaar"));
    }

    #[test]
    fn test_record_set_array_field() {
        // Record set returning a list of all matching records under a single
        // input name, for FOREACH iteration.
        let records = vec![
            {
                let mut r = BTreeMap::new();
                r.insert("ouder_bsn".to_string(), Value::String("123".to_string()));
                r.insert("kind_naam".to_string(), Value::String("Anna".to_string()));
                r.insert("leeftijd".to_string(), Value::Int(8));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("ouder_bsn".to_string(), Value::String("123".to_string()));
                r.insert("kind_naam".to_string(), Value::String("Bram".to_string()));
                r.insert("leeftijd".to_string(), Value::Int(11));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("ouder_bsn".to_string(), Value::String("456".to_string()));
                r.insert("kind_naam".to_string(), Value::String("Cees".to_string()));
                r.insert("leeftijd".to_string(), Value::Int(4));
                r
            },
        ];

        let source = RecordSetDataSource::builder("kinderen", 10)
            .select_on(vec!["ouder_bsn".to_string()])
            .array_field("kinderen_data", &["kind_naam", "leeftijd"])
            .records(records)
            .build()
            .unwrap();

        let mut criteria = BTreeMap::new();
        criteria.insert("ouder_bsn".to_string(), Value::String("123".to_string()));

        let result = source.get("kinderen_data", &criteria);
        match result {
            Some(Value::Array(items)) => {
                assert_eq!(items.len(), 2);
                if let Value::Object(o) = &items[0] {
                    assert_eq!(o.get("kind_naam"), Some(&Value::String("Anna".to_string())));
                    assert_eq!(o.get("leeftijd"), Some(&Value::Int(8)));
                    assert_eq!(o.len(), 2, "Only projected fields should be present");
                } else {
                    panic!("Expected object item");
                }
            }
            other => panic!("Expected array, got {:?}", other),
        }

        // Field index should advertise the array field
        assert!(source.has_field("kinderen_data"));
    }

    #[test]
    fn test_record_set_array_field_no_projection() {
        // No projection: include the whole record
        let records = vec![{
            let mut r = BTreeMap::new();
            r.insert("ouder_bsn".to_string(), Value::String("123".to_string()));
            r.insert("kind_naam".to_string(), Value::String("Anna".to_string()));
            r
        }];

        let source = RecordSetDataSource::builder("kinderen", 10)
            .select_on(vec!["ouder_bsn".to_string()])
            .array_field("kinderen", &[])
            .records(records)
            .build()
            .unwrap();

        let mut criteria = BTreeMap::new();
        criteria.insert("ouder_bsn".to_string(), Value::String("123".to_string()));
        match source.get("kinderen", &criteria) {
            Some(Value::Array(items)) => {
                assert_eq!(items.len(), 1);
                if let Value::Object(o) = &items[0] {
                    assert_eq!(o.len(), 2, "Whole record should be present");
                }
            }
            other => panic!("Expected array, got {:?}", other),
        }
    }

    #[test]
    fn test_record_set_no_match_returns_none() {
        let source = RecordSetDataSource::builder("persons", 10)
            .key_field("bsn")
            .records(make_records())
            .build()
            .unwrap();

        let mut criteria = BTreeMap::new();
        criteria.insert("bsn".to_string(), Value::String("999".to_string()));
        assert_eq!(source.get("income", &criteria), None);
    }

    #[test]
    fn test_record_set_extra_criteria_ignored_with_key_field() {
        let source = RecordSetDataSource::builder("persons", 10)
            .key_field("bsn")
            .records(make_records())
            .build()
            .unwrap();

        // year is irrelevant when only bsn is the key — should still match the
        // first record with bsn 123 (selecting the first matching row).
        let mut criteria = BTreeMap::new();
        criteria.insert("bsn".to_string(), Value::String("123".to_string()));
        criteria.insert("unrelated".to_string(), Value::Int(9999));
        assert!(source.get("income", &criteria).is_some());
    }

    #[test]
    fn test_record_set_via_registry_priority() {
        let mut registry = DataSourceRegistry::new();
        registry.add_source(Box::new(
            RecordSetDataSource::builder("persons", 10)
                .key_field("bsn")
                .records(make_records())
                .build()
                .unwrap(),
        ));

        let mut criteria = BTreeMap::new();
        criteria.insert("bsn".to_string(), Value::String("456".to_string()));
        let result = registry.resolve("income", &criteria).unwrap();
        assert_eq!(result.value, Value::Int(40000));
        assert_eq!(result.source_type, "record_set");
    }

    #[test]
    fn test_record_set_builder_requires_records() {
        let result = RecordSetDataSource::builder("persons", 10).build();
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // query_native (YAML-driven) tests
    // -------------------------------------------------------------------------

    fn make_yaml_records() -> Vec<BTreeMap<String, Value>> {
        vec![
            {
                let mut r = BTreeMap::new();
                r.insert("bsn".to_string(), Value::String("123".to_string()));
                r.insert("year".to_string(), Value::Int(2024));
                r.insert("income".to_string(), Value::Int(50000));
                r.insert("partner_bsn".to_string(), Value::String("999".to_string()));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("bsn".to_string(), Value::String("123".to_string()));
                r.insert("year".to_string(), Value::Int(2025));
                r.insert("income".to_string(), Value::Int(55000));
                r.insert("partner_bsn".to_string(), Value::String("888".to_string()));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("bsn".to_string(), Value::String("456".to_string()));
                r.insert("year".to_string(), Value::Int(2025));
                r.insert("income".to_string(), Value::Int(40000));
                r
            },
        ]
    }

    #[test]
    fn test_query_native_scalar_field_with_select_on() {
        // Engine reads YAML metadata: table=persons, field=income, select_on=[bsn,year]
        let source = RecordSetDataSource::builder("persons", 10)
            .records(make_yaml_records())
            .build()
            .unwrap();

        let select = vec![
            SelectOn {
                field: "bsn".to_string(),
                value: Value::String("123".to_string()),
            },
            SelectOn {
                field: "year".to_string(),
                value: Value::Int(2025),
            },
        ];
        let result = source.query_native(Some("income"), None, &select, &BTreeMap::new(), false);
        assert_eq!(result, Some(Value::Int(55000)));
    }

    #[test]
    fn test_query_native_object_fields_projection() {
        let source = RecordSetDataSource::builder("persons", 10)
            .records(make_yaml_records())
            .build()
            .unwrap();

        let select = vec![SelectOn {
            field: "bsn".to_string(),
            value: Value::String("123".to_string()),
        }];
        let result = source.query_native(
            None,
            Some(&["bsn".to_string(), "income".to_string()]),
            &select,
            &BTreeMap::new(),
            false,
        );
        match result {
            Some(Value::Object(obj)) => {
                assert_eq!(obj.len(), 2, "Only projected fields should be present");
                assert_eq!(obj.get("bsn"), Some(&Value::String("123".to_string())));
                assert!(obj.get("income").is_some());
            }
            other => panic!("Expected object, got {:?}", other),
        }
    }

    #[test]
    fn test_query_native_array_mode() {
        // Array input: return all matching rows for FOREACH
        let records = vec![
            {
                let mut r = BTreeMap::new();
                r.insert("ouder_bsn".to_string(), Value::String("123".to_string()));
                r.insert("naam".to_string(), Value::String("Anna".to_string()));
                r.insert("leeftijd".to_string(), Value::Int(8));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("ouder_bsn".to_string(), Value::String("123".to_string()));
                r.insert("naam".to_string(), Value::String("Bram".to_string()));
                r.insert("leeftijd".to_string(), Value::Int(11));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("ouder_bsn".to_string(), Value::String("999".to_string()));
                r.insert("naam".to_string(), Value::String("Cees".to_string()));
                r.insert("leeftijd".to_string(), Value::Int(4));
                r
            },
        ];

        let source = RecordSetDataSource::builder("kinderen", 10)
            .records(records)
            .build()
            .unwrap();

        let select = vec![SelectOn {
            field: "ouder_bsn".to_string(),
            value: Value::String("123".to_string()),
        }];

        let result = source.query_native(
            None,
            Some(&["naam".to_string(), "leeftijd".to_string()]),
            &select,
            &BTreeMap::new(),
            true,
        );

        match result {
            Some(Value::Array(items)) => {
                assert_eq!(items.len(), 2);
                if let Value::Object(o) = &items[0] {
                    assert_eq!(o.get("naam"), Some(&Value::String("Anna".to_string())));
                    assert_eq!(o.get("leeftijd"), Some(&Value::Int(8)));
                }
            }
            other => panic!("Expected array, got {:?}", other),
        }
    }

    #[test]
    fn test_query_native_no_match() {
        let source = RecordSetDataSource::builder("persons", 10)
            .records(make_yaml_records())
            .build()
            .unwrap();

        let select = vec![SelectOn {
            field: "bsn".to_string(),
            value: Value::String("nonexistent".to_string()),
        }];
        let result = source.query_native(Some("income"), None, &select, &BTreeMap::new(), false);
        assert!(result.is_none());
    }

    #[test]
    fn test_query_native_loose_numeric_compare() {
        // year is stored as Int(2025); criterion is String("2025") — should match
        let source = RecordSetDataSource::builder("persons", 10)
            .records(make_yaml_records())
            .build()
            .unwrap();

        let select = vec![
            SelectOn {
                field: "bsn".to_string(),
                value: Value::String("123".to_string()),
            },
            SelectOn {
                field: "year".to_string(),
                value: Value::String("2025".to_string()),
            },
        ];
        let result = source.query_native(Some("income"), None, &select, &BTreeMap::new(), false);
        assert_eq!(result, Some(Value::Int(55000)));
    }

    #[test]
    fn test_registry_resolve_native() {
        let mut registry = DataSourceRegistry::new();
        registry.add_source(Box::new(
            RecordSetDataSource::builder("persons", 10)
                .records(make_yaml_records())
                .build()
                .unwrap(),
        ));

        let select = vec![
            SelectOn {
                field: "bsn".to_string(),
                value: Value::String("456".to_string()),
            },
            SelectOn {
                field: "year".to_string(),
                value: Value::Int(2025),
            },
        ];
        let result = registry
            .resolve_native(
                "persons",
                Some("income"),
                None,
                &select,
                &BTreeMap::new(),
                false,
            )
            .unwrap();
        assert_eq!(result.value, Value::Int(40000));
        assert_eq!(result.source_name, "persons");
        assert_eq!(result.source_type, "record_set");
    }

    #[test]
    fn test_registry_resolve_native_unknown_table() {
        let registry = DataSourceRegistry::new();
        let result =
            registry.resolve_native("missing", Some("foo"), None, &[], &BTreeMap::new(), false);
        assert!(result.is_none());
    }

    #[test]
    fn test_record_set_alias_and_select_on_combined() {
        let source = RecordSetDataSource::builder("persons", 10)
            .select_on(vec!["bsn".to_string(), "year".to_string()])
            .alias("advies_mate_van_gevaar", "mate_van_gevaar")
            .records(make_records())
            .build()
            .unwrap();

        let mut criteria = BTreeMap::new();
        criteria.insert("bsn".to_string(), Value::String("123".to_string()));
        criteria.insert("year".to_string(), Value::Int(2025));
        assert_eq!(
            source.get("advies_mate_van_gevaar", &criteria),
            Some(Value::String("middel".to_string()))
        );
    }

    #[test]
    fn test_from_records_multi_criteria_lookup() {
        // from_records stores by a single key_field, but get() receives
        // all criteria. Without key_fields filtering, the extra criteria
        // would cause a key mismatch and the lookup would silently fail.
        let records = vec![
            {
                let mut r = BTreeMap::new();
                r.insert("BSN".to_string(), Value::String("123".to_string()));
                r.insert("income".to_string(), Value::Int(50000));
                r
            },
            {
                let mut r = BTreeMap::new();
                r.insert("BSN".to_string(), Value::String("456".to_string()));
                r.insert("income".to_string(), Value::Int(40000));
                r
            },
        ];

        let source = DictDataSource::from_records("persons", 10, "BSN", records).unwrap();

        // Lookup with multiple criteria — the extra "year" criterion should be
        // ignored because the source was created with key_field="BSN"
        let mut criteria = BTreeMap::new();
        criteria.insert("BSN".to_string(), Value::String("123".to_string()));
        criteria.insert("year".to_string(), Value::Int(2025));

        assert_eq!(source.get("income", &criteria), Some(Value::Int(50000)));

        // Single criterion should still work
        let mut criteria_single = BTreeMap::new();
        criteria_single.insert("BSN".to_string(), Value::String("456".to_string()));
        assert_eq!(
            source.get("income", &criteria_single),
            Some(Value::Int(40000))
        );
    }
}
