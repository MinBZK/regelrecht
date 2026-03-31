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
