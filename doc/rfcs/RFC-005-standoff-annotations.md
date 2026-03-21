# RFC-005: Stand-off Annotations for Legal Texts

**Status:** Proposed
**Date:** 2025-12-16
**Authors:** Anne Schuth

## Context

Legal texts are stored as verbatim text in YAML files. We want to add annotations
at word or character level, without modifying the legal text itself. Annotations
must be version-resilient: when text changes or moves, an annotation should
automatically find its new location. Annotations should resolve on
**any version** of a law where the annotated text exists - both older and newer
versions - without requiring migration logic or change tracking.

**Scope:** This RFC defines the annotation *format*, not storage. Annotations
could be stored in a central database, distributed across systems, or provided
by external parties ("bring your own annotations"). The format is intentionally
storage-agnostic.

## Decision

We adopt the [W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/), a
W3C Recommendation since 2017. This standard defines a common format for annotations
on the web, used by tools like Hypothesis, Apache Annotator, and Recogito.

Specifically, we use **TextQuoteSelector** from the
[W3C Selectors and States](https://www.w3.org/TR/selectors-states/) specification.
The selector refers to text via an exact quote plus context (prefix/suffix).

```yaml
selector:
  type: TextQuoteSelector
  exact: zorgtoeslag
  prefix: "heeft de verzekerde aanspraak op een "
  suffix: " ter grootte van dat verschil"
```

### Why This Works

The TextQuoteSelector is **self-locating**: the text itself (with context) is the
identifier, not an article number or character position.

**Scenario: Article is renumbered**

A new article 1a is inserted, causing article 2 to be renumbered to article 3.
The content of the article remains identical.

| Selector type | What happens? |
|---------------|---------------|
| `article[number='2']` | ❌ Breaks - article 2 no longer exists |
| `TextPositionSelector(start=245)` | ❌ Breaks - positions have shifted |
| `TextQuoteSelector("zorgtoeslag", prefix="aanspraak op een ")` | ✅ Finds the text in article 3 |

The TextQuoteSelector searches for text in the entire document. It doesn't matter
where that text is located - if the prefix/suffix/exact combination is unique,
the annotation resolves correctly.

**Scenario: Viewing annotations across law versions**

An annotation created today should also be visible when viewing an older version
of the law (e.g., the 2020 version), as long as the annotated text existed then.

| Approach | Annotation today → view on 2020 version | Complexity |
|----------|----------------------------------------|------------|
| Article + version + change tracking | ❌ Requires reverse migration of all changes | High |
| TextQuoteSelector | ✅ Automatic - just search for the text | Low |

TextQuoteSelector is **content-addressed**: the annotation finds text by its
content, not by its structural location. This means:
- An annotation made today automatically resolves on older law versions (if the text existed)
- No migration logic needed when laws change
- Works bidirectionally in time without extra effort

### Example Legal Text

Given this fragment from Zorgtoeslagwet article 2:

```yaml
- number: '2'
  text: |-
    1. Indien de normpremie voor een verzekerde in het berekeningsjaar minder
    bedraagt dan de standaardpremie in dat jaar, heeft de verzekerde aanspraak
    op een zorgtoeslag ter grootte van dat verschil.
```

### Example 1: Textual Comment

A legal expert explains what "zorgtoeslag" means:

```yaml
type: Annotation
motivation: commenting
target:
  source: regelrecht://zorgtoeslagwet
  selector:
    type: TextQuoteSelector
    exact: zorgtoeslag
    prefix: "heeft de verzekerde aanspraak op een "
    suffix: " ter grootte van dat verschil"
body:
  type: TextualBody
  value: "This is the monthly allowance for health insurance costs."
  purpose: commenting
  format: text/plain
  language: en
```

### Example 2: Link to Machine-Readable Execution

The interpreter links text to the calculation:

```yaml
type: Annotation
motivation: linking
target:
  source: regelrecht://zorgtoeslagwet
  selector:
    type: TextQuoteSelector
    exact: zorgtoeslag ter grootte van dat verschil
    prefix: "heeft de verzekerde aanspraak op een "
    suffix: ". Voor een verzekerde"
body:
  type: SpecificResource
  source: regelrecht://zorgtoeslagwet/bereken_zorgtoeslag#hoogte_zorgtoeslag
  purpose: linking
```

### Example 3: Tag/Classification

An analyst classifies legal concepts:

```yaml
type: Annotation
motivation: tagging
target:
  source: regelrecht://zorgtoeslagwet
  selector:
    type: TextQuoteSelector
    exact: verzekerde
    prefix: "heeft de "
    suffix: " aanspraak op een zorgtoeslag"
body:
  type: TextualBody
  value: legal-subject
  purpose: tagging
```

## Fuzzy Matching

When the exact text is no longer found (e.g., due to a minor textual change),
fuzzy matching can still resolve the annotation.

### How It Works

1. **Exact match** - Search for `prefix + exact + suffix` literally in the text
2. **Fuzzy match** - If exact match fails, search with similarity scoring

### Example

**Original text:**
```
heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil
```

**Changed text (after amendment):**
```
heeft de verzekerde recht op een zorgtoeslag ter grootte van het verschil
```

The annotation searches for:
- prefix: `"heeft de verzekerde "`
- exact: `"aanspraak op een zorgtoeslag"`
- suffix: `" ter grootte van dat verschil"`

**Fuzzy matching finds the best candidate:**

```
Candidate: "recht op een zorgtoeslag"
           ─────────────────────────
Score calculation:
  - exact similarity:  "aanspraak op een zorgtoeslag" vs "recht op een zorgtoeslag"
                       Levenshtein: 9 edits / 28 chars = 0.68 similarity
  - prefix match:      "heeft de verzekerde " ✓ (exact match = 1.0)
  - suffix similarity: "ter grootte van dat verschil" vs "ter grootte van het verschil"
                       Levenshtein: 1 edit / 29 chars = 0.97 similarity

Weighted score: (0.68 × 0.5) + (1.0 × 0.25) + (0.97 × 0.25) = 0.83
```

With a threshold of 0.7, this match is accepted.

### Pseudocode

```python
def resolve_annotation(text: str, selector: TextQuoteSelector) -> Match | None:
    # Step 1: Try exact match
    pattern = selector.prefix + selector.exact + selector.suffix
    if pattern in text:
        start = text.index(pattern) + len(selector.prefix)
        return Match(start=start, end=start + len(selector.exact), confidence=1.0)

    # Step 2: Fuzzy matching
    best_match = None
    best_score = 0

    for candidate in find_candidates(text, selector.exact):
        # Get context around the candidate
        prefix_in_text = text[candidate.start - len(selector.prefix):candidate.start]
        suffix_in_text = text[candidate.end:candidate.end + len(selector.suffix)]

        # Calculate similarity scores
        exact_score = levenshtein_similarity(selector.exact, candidate.text)
        prefix_score = levenshtein_similarity(selector.prefix, prefix_in_text)
        suffix_score = levenshtein_similarity(selector.suffix, suffix_in_text)

        # Weighted score: exact counts more than context
        score = (exact_score * 0.5) + (prefix_score * 0.25) + (suffix_score * 0.25)

        if score > best_score:
            best_score = score
            best_match = candidate

    if best_score >= THRESHOLD:  # e.g., 0.7
        return Match(start=best_match.start, end=best_match.end, confidence=best_score)

    return None  # Annotation could not be resolved
```

### When Fuzzy Matching Fails

For large text changes (score < threshold), the annotation is marked as "orphaned".
The annotation is preserved with its original context, so that:
- Users can see what was annotated
- Manual re-anchoring is possible
- The annotation history is preserved

## Implementation Notes

### Performance

Fuzzy matching through an entire law can be expensive. Recommended strategy:

1. **Exact match first** - Search for `prefix + exact + suffix` as a simple substring.
   This succeeds in 99% of cases and is O(n).

2. **Optional hint** - Add `regelrecht:hint` with a W3C selector as optimization.
   The hint is non-authoritative: if the text doesn't match there, search the
   entire law.

   Position offsets are relative to an article, not the entire law. Therefore,
   use `refinedBy` to combine a TextPositionSelector with a CssSelector:

   ```yaml
   type: TextQuoteSelector
   exact: zorgtoeslag
   prefix: "aanspraak op een "
   suffix: " ter grootte"
   regelrecht:hint:
     type: CssSelector
     value: "article[number='2']"
     refinedBy:
       type: TextPositionSelector
       start: 45
       end: 56
   ```

   This says: "look first in article 2 at position 45-56". If that doesn't match
   (article renumbered or text changed), then search the entire law.

   **Resolution algorithm with hint:**
   1. Evaluate the hint (article 2, position 45-56)
   2. Search for TextQuoteSelector within that search space
   3. Not found? → search entire law (hint was outdated)

3. **Caching** - Cache resolved positions per `(annotation_id, law_version)`.
   Invalidate only when a new law version is published.

### Uniqueness

A selector must match uniquely within the law. Multiple matches make the
annotation ambiguous and unreliable to resolve.

**When creating an annotation:**
- Validate that the selector is unique in the current law version
- If not: error message "add more context to prefix/suffix"

**When resolving an annotation:**
- If there are multiple matches with equal score: mark as "ambiguous"
- Let the user choose or manually re-anchor the annotation

**Rule of thumb:** prefix and suffix of ~30-50 characters are usually sufficient
to be unique, even for common words.

## Why

### Benefits

1. **Version resilience**: TextQuoteSelector finds text regardless of where it is
2. **Renumbering-proof**: Article numbers can change without breaking annotations
3. **Fuzzy matching**: Minor text changes are automatically handled
4. **No changes to legal text**: Annotations are completely separate from source text
5. **W3C standard**: Interoperable with existing annotation tools (Hypothesis, etc.)
6. **Simple**: One selector type, no complex fallback logic needed

### Tradeoffs

- Prefix/suffix must be long enough to be unique within the law (~20-50 characters)
- Fuzzy matching can fail for large changes (annotation becomes "orphaned")
- Resolution requires searching through the entire text (no direct lookup)

### Alternatives Considered

**Article + version with change tracking**
- Requires explicitly modeling every type of legal change (renumbering, amendments, etc.)
- Annotations become version-bound; viewing across versions requires migration logic
- Higher complexity for a problem TextQuoteSelector solves automatically

**CssSelector for article scope**
- Breaks when articles are renumbered
- Adds no value if TextQuoteSelector is already unique

**TextPositionSelector (character offsets)**
- Too brittle: any text change breaks all annotations
- No fuzzy matching possible

**Inline anchors in the text**
- Modifies the verbatim legal text, not acceptable

## Schema

### TextQuoteSelector

```yaml
# JSON Schema for TextQuoteSelector with regelrecht:hint extension
type: object
required: [type, exact]
properties:
  type:
    const: TextQuoteSelector
  exact:
    type: string
    description: The exact text to match
  prefix:
    type: string
    description: Text immediately before the exact match (for disambiguation)
  suffix:
    type: string
    description: Text immediately after the exact match (for disambiguation)
  regelrecht:hint:
    type: object
    description: Optional performance hint (non-authoritative)
    properties:
      type:
        const: CssSelector
      value:
        type: string
        pattern: "^article\\[number='[^']+']$"
        description: CSS selector for the article (e.g., "article[number='2']")
      refinedBy:
        type: object
        properties:
          type:
            const: TextPositionSelector
          start:
            type: integer
            minimum: 0
          end:
            type: integer
            minimum: 0
```

### Annotation

```yaml
# JSON Schema for Annotation
type: object
required: [type, target, motivation]
properties:
  type:
    const: Annotation
  motivation:
    type: string
    description: |
      W3C motivation - why this annotation exists.
      W3C: optional (SHOULD). Regelrecht: required.
    enum:
      # W3C standard vocabulary:
      - assessing       # Quality assessment
      - bookmarking     # Bookmark for later
      - classifying     # Formal classification
      - commenting      # Human explanation or note
      - describing      # Metadata description
      - editing         # Request or suggest edit
      - highlighting    # Visual emphasis
      - identifying     # Identify the target
      - linking         # Link to another resource
      - moderating      # Moderation action
      - questioning     # Open question
      - replying        # Reply to another annotation
      - tagging         # Classification tag
  resolution:
    type: string
    description: Whether the selector found the text
    enum:
      - found           # Text located successfully
      - orphaned        # Text not found in current law version
    default: found
  workflow:
    type: string
    description: Workflow status (for questioning/reviewing motivations)
    enum:
      - open            # Needs attention
      - resolved        # Issue addressed
    default: open
  target:
    type: object
    required: [source, selector]
    properties:
      source:
        type: string
        format: uri
        description: URI of the law (e.g., regelrecht://zorgtoeslagwet)
      selector:
        $ref: "#/TextQuoteSelector"
  body:
    oneOf:
      - type: object  # TextualBody
        required: [type, value, purpose]
        properties:
          type: { const: TextualBody }
          value: { type: string }
          purpose:
            type: string
            description: |
              W3C purpose - role of this body. Same vocabulary as motivation.
              W3C: optional (MAY). Regelrecht: required.
            enum: [assessing, bookmarking, classifying, commenting, describing,
                   editing, highlighting, identifying, linking, moderating,
                   questioning, replying, tagging]
          format: { type: string }
          language: { type: string }
      - type: object  # SpecificResource (link)
        required: [type, source, purpose]
        properties:
          type: { const: SpecificResource }
          source: { type: string, format: uri }
          purpose:
            type: string
            description: W3C purpose - same vocabulary as motivation
            enum: [assessing, bookmarking, classifying, commenting, describing,
                   editing, highlighting, identifying, linking, moderating,
                   questioning, replying, tagging]
```

### Field Semantics

| Field | Dimension | Values | Description |
|-------|-----------|--------|-------------|
| `motivation` | Intent | commenting, linking, tagging, ... | W3C standard: why does this annotation exist? |
| `resolution` | Technical | found, orphaned | Can the selector locate the text? |
| `workflow` | Process | open, resolved | Has the issue been addressed? |

`motivation` and `purpose` use the [W3C Web Annotation vocabulary](https://www.w3.org/TR/annotation-model/#motivation-and-purpose) (13 values). W3C makes these optional; we require them for explicit intent.

`resolution` and `workflow` are orthogonal: an annotation can be `orphaned` + `resolved`.

## References

- [W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)
- [W3C Selectors and States](https://www.w3.org/TR/selectors-states/)
- [Hypothesis Fuzzy Anchoring](https://web.hypothes.is/blog/fuzzy-anchoring/)
- [Google diff-match-patch](https://github.com/google/diff-match-patch) - fuzzy matching library
- [Glossary of Dutch Legal Terms](../glossary.md)
