# Issue: Article ID Collision (420bis.1)

**Status**: Open
**Priority**: High
**Scope**: Future PR (too large for current harvester PR)

---

## Problem Description

The current article numbering system uses a flat string representation that joins number parts with a dot (`.`) separator. This causes collisions when:

1. An article has a dot in its original designation (e.g., `Artikel 420bis.1`)
2. Another article with a lid creates the same string (e.g., `Artikel 420bis` lid `1`)

### Example: Wetboek van Strafrecht

In the Wetboek van Strafrecht, both of these exist:
- **Artikel 420bis** with **lid 1** → currently generates `420bis.1`
- **Artikel 420bis.1** (separate article for "eenvoudig witwassen") → also generates `420bis.1`

Same collision occurs with:
- **Artikel 420quater** lid 1 vs **Artikel 420quater.1**

### Current Implementation

```rust
// splitting/types.rs:208
pub fn to_number(&self) -> String {
    let base = self.number_parts.join(".");  // ← Collision point
    match &self.bijlage_prefix {
        Some(prefix) => format!("{prefix}:{base}"),
        None => base,
    }
}
```

The `number_parts` vector:
- `["420bis", "1"]` for artikel 420bis, lid 1 → `"420bis.1"`
- `["420bis.1"]` for artikel 420bis.1 → `"420bis.1"`

These produce identical strings, making them indistinguishable.

### Why Simple Separator Changes Don't Work

Changing the separator from `.` to `_` or `-` doesn't solve the problem:
- Dutch law article numbers can contain any character
- There's no guarantee that `_` or `-` won't appear in article designations
- This just shifts the collision problem to different articles

---

## Proposed Solutions

### Option A: Article-Level Marker (Simpler)

Keep dot notation for hierarchical splits, but add a marker/suffix to article-level entries that happen to contain dots in their original designation.

**Principle**: Dot notation is "owned" by the hierarchical split (artikel → lid → onderdeel). If an article's original name contains a dot, it gets marked to distinguish it.

**Example**:
```yaml
# Artikel 420bis, lid 1 (hierarchical split)
- number: "420bis.1"
  text: "Lid 1 van artikel 420bis..."

# Artikel 420bis.1 (original article name contains dot)
- number: "420bis.1§"   # § suffix marks this as article-level
  text: "Artikel 420bis.1 (eenvoudig witwassen)..."
```

**Alternative markers**:
- `420bis.1§` - section symbol (uncommon in Dutch law)
- `420bis.1#` - hash suffix
- `420bis.1!` - exclamation suffix
- `[420bis.1]` - brackets around entire number

**Implementation**:
```rust
pub fn to_number(&self) -> String {
    let base = self.number_parts.join(".");

    // If single-part number contains a dot, mark it as article-level
    let needs_marker = self.number_parts.len() == 1
        && self.number_parts[0].contains('.');

    let marked = if needs_marker {
        format!("{base}§")
    } else {
        base
    };

    match &self.bijlage_prefix {
        Some(prefix) => format!("{prefix}:{marked}"),
        None => marked,
    }
}
```

**Pros**:
- Minimal change to current system
- No schema change required
- Backwards compatible (most articles unaffected)
- Clear convention: marker = article-level, no marker = could be hierarchical

**Cons**:
- Introduces a special case
- Consumers need to understand the marker convention
- Display logic needs to strip marker for human-readable output

---

### Option B: Structured Article ID (More Robust)

Instead of a flat string, represent article IDs as a structured object with explicit hierarchy.

**Principle**: Make hierarchy explicit in the data structure, not encoded in a string.

### Schema Change

```yaml
# Current (flat string)
articles:
  - number: "420bis.1"
    text: "..."

# Proposed (structured object)
articles:
  - id:
      artikel: "420bis"
      lid: "1"
    text: "..."

  - id:
      artikel: "420bis.1"
    text: "..."
```

### Rust Type

```rust
/// Structured article identifier with explicit hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleId {
    /// Original article number from source (e.g., "420bis", "420bis.1", "8:36c")
    pub artikel: String,

    /// Lid number if this is a sub-article (e.g., "1", "2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lid: Option<String>,

    /// Onderdeel (list item) if this is a sub-lid (e.g., "a", "b", "1°")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onderdeel: Option<String>,

    /// Bijlage prefix if in an appendix (e.g., "B1", "B2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bijlage: Option<String>,
}

impl ArticleId {
    /// Generate a unique string representation for lookups.
    /// Uses a separator that cannot appear in Dutch law (e.g., "§" or "|").
    pub fn to_key(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref b) = self.bijlage {
            parts.push(b.as_str());
        }
        parts.push(&self.artikel);
        if let Some(ref l) = self.lid {
            parts.push(l.as_str());
        }
        if let Some(ref o) = self.onderdeel {
            parts.push(o.as_str());
        }
        parts.join("§")  // § cannot appear in Dutch law article numbers
    }

    /// Human-readable display format.
    pub fn display(&self) -> String {
        let mut result = String::new();
        if let Some(ref b) = self.bijlage {
            result.push_str(b);
            result.push(':');
        }
        result.push_str(&self.artikel);
        if let Some(ref l) = self.lid {
            result.push_str(" lid ");
            result.push_str(l);
        }
        if let Some(ref o) = self.onderdeel {
            result.push_str(", onderdeel ");
            result.push_str(o);
        }
        result
    }
}
```

### Benefits

1. **No collisions**: Each hierarchy level is explicit, not encoded in a string
2. **Queryable**: Can find "all leden of artikel 420bis" without string parsing
3. **Self-documenting**: Clear what each part represents
4. **Extensible**: Easy to add more hierarchy levels (sub-onderdelen, etc.)
5. **Display flexibility**: Can format for different contexts (legal citation, URL fragment, etc.)

### Migration Path

1. Update JSON schema to support structured `id` object
2. Modify harvester to generate structured IDs
3. Update engine to consume structured IDs
4. Optionally support both formats during transition

---

## Impact Assessment

### Files Affected

**Harvester**:
- `packages/harvester/src/splitting/types.rs` - Replace `number_parts: Vec<String>` with `ArticleId`
- `packages/harvester/src/types.rs` - Update `Article` struct
- `packages/harvester/src/yaml/writer.rs` - Serialize structured ID
- All tests using number assertions

**Schema**:
- JSON schema needs `id` object definition
- YAML files need migration or dual-format support

**Engine** (if affected):
- Article lookup by ID
- Cross-reference resolution

### Scope

This is a **breaking change** that affects:
- Schema definition
- All existing YAML law files
- Harvester output format
- Engine article resolution

**Recommendation**: Implement in a dedicated PR with proper RFC process.

---

## Solution Comparison

| Aspect | Option A (Marker) | Option B (Structured) |
|--------|-------------------|----------------------|
| **Complexity** | Low | High |
| **Schema change** | No | Yes |
| **Breaking change** | Minimal | Yes |
| **Future-proof** | Moderate | High |
| **Implementation effort** | ~1 day | ~1 week |
| **Query capability** | String parsing | Native |

**Recommendation**: Start with **Option A** for the current PR if a quick fix is needed. Plan **Option B** for a future iteration when the schema can be properly versioned.

---

## Temporary Workarounds

Until the structural solution is implemented:

1. **Document known collisions**: List affected articles in Wetboek van Strafrecht
2. **Manual post-processing**: Flag/fix collisions after harvesting
3. **Avoid splitting colliding articles**: Keep artikel 420bis as single unit

---

## References

- **Current location**: `packages/harvester/src/splitting/types.rs:206-213`
- **Affected laws**: Wetboek van Strafrecht (BWBR0001854)
- **Colliding articles**: 420bis/420bis.1, 420quater/420quater.1
