# Execution Engine

The execution engine is the core of RegelRecht: a deterministic Rust runtime that evaluates machine-readable Dutch law.

## Overview

- **Language**: Rust
- **Location**: `packages/engine/`
- **Targets**: Native (x86/ARM) and WebAssembly (browser/Node.js)
- **Key property**: Deterministic - same inputs always produce the same outputs

## Architecture

The engine has a layered architecture:

```mermaid
flowchart TD
    A[LawExecutionService] -->|orchestrates| B[RuleResolver]
    A -->|delegates| C[ArticleEngine]
    C -->|uses| D[RuleContext]
    C -->|calls| E[Operations]
    A -->|recurses via| F[ServiceProvider]
    F -->|cross-law| A
    B -->|indexes| G[Law Registry]
    D -->|resolves| H[Variable Chain]
```

| Module | Purpose |
|--------|---------|
| `service.rs` | `LawExecutionService` - top-level API, cross-law orchestration |
| `engine.rs` | `ArticleEngine` - single article execution |
| `resolver.rs` | `RuleResolver` - law registry, output→article indexing, IoC lookup |
| `context.rs` | `RuleContext` - execution state, variable resolution with priority chain |
| `operations.rs` | 21 operation types (arithmetic, comparison, logical, conditional, date) |
| `uri.rs` | `regelrecht://` URI parsing for cross-law references |
| `trace.rs` | Execution tracing with box-drawing visualization |
| `priority.rs` | Lex superior / lex posterior resolution for competing implementations |
| `data_source.rs` | External data registry for non-law data lookups |
| `config.rs` | Security limits (max laws, YAML size, recursion depth) |

## How It Works

```mermaid
flowchart TD
    A[Load Law YAML] --> B[Parse Articles]
    B --> C[Build Output Index]
    C --> D[Resolve Inputs]
    D --> E{Cross-Law Reference?}
    E -->|Yes| F[Load & Execute Referenced Law]
    F --> D
    E -->|No| G[Resolve Open Terms via IoC]
    G --> H[Execute Operations]
    H --> I[Produce Outputs with Trace]
```

### Variable Resolution Priority

When the engine resolves a `$variable`, it checks these sources in order:

1. **Context variables** - `referencedate`, `referencedate.year`, etc.
2. **Local scope** - loop variables from `FOREACH`
3. **Outputs** - values calculated by previous actions in the same article
4. **Resolved inputs** - cached results from cross-law references
5. **Definitions** - article-level constants
6. **Parameters** - direct input parameters

## Operations

The engine supports 21 operations for expressing legal logic:

| Category | Operations |
|----------|-----------|
| **Comparison** | `EQUALS`, `NOT_EQUALS`, `GREATER_THAN`, `LESS_THAN`, `GREATER_THAN_OR_EQUAL`, `LESS_THAN_OR_EQUAL` |
| **Arithmetic** | `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE` |
| **Aggregate** | `MAX`, `MIN` |
| **Logical** | `AND`, `OR` |
| **Conditional** | `IF` (when/then/else), `SWITCH` (cases/default) |
| **Null checking** | `IS_NULL`, `NOT_NULL` |
| **Membership** | `IN`, `NOT_IN` |
| **Date** | `SUBTRACT_DATE` (with unit: days/months/years) |

See [RFC-004](/rfcs/rfc-004) for the full specification.

## Cross-Law Execution

Laws reference each other via `source` on input fields:

```yaml
input:
  - name: standaardpremie
    source:
      regulation: regeling_standaardpremie
      output: standaardpremie
      parameters:
        bsn: $bsn
```

The engine automatically loads the referenced law, executes it with the specified parameters, and caches the result. Circular references are detected and raise an error.

### Open Term Resolution (IoC)

Higher laws declare `open_terms` that lower regulations fill via `implements`. At execution time, the engine:

1. Indexes all `implements` declarations at law load time
2. Looks up implementations for each `open_term`
3. Filters by temporal validity (`calculation_date`) and scope (`gemeente_code`, etc.)
4. Resolves conflicts via **lex superior** (higher layer wins) then **lex posterior** (newer date wins)
5. Falls back to the `default` if no implementation found

See [RFC-003](/rfcs/rfc-003) for the full pattern.

## Execution Tracing

Every execution can produce a full trace tree showing how each value was computed:

```rust
let result = service.evaluate_law_output_with_trace(
    "zorgtoeslagwet", "hoogte_zorgtoeslag", params, "2025-01-01"
)?;

if let Some(trace) = result.trace {
    println!("{}", trace.render_box_drawing());
}
```

The trace includes: which articles were executed, which inputs were resolved (and from where), which operations ran, and the result of each step.

## WASM Usage

The engine compiles to WebAssembly for browser and Node.js execution.

### Browser

```javascript
import init, { WasmEngine } from 'regelrecht-engine';

await init();
const engine = new WasmEngine();

const lawId = engine.loadLaw(yamlString);
const result = engine.execute(
    lawId,
    'heeft_recht_op_zorgtoeslag',
    { BSN: '123456789', vermogen: 50000 },
    '2025-01-01'
);

console.log(result.outputs);
```

### Node.js

```javascript
import { initSync, WasmEngine } from 'regelrecht-engine';
import { readFileSync } from 'fs';

const wasmBuffer = readFileSync('./regelrecht_engine_bg.wasm');
initSync({ module: wasmBuffer });

const engine = new WasmEngine();
// Same API as browser
```

### WASM API

```typescript
engine.loadLaw(yaml: string): string
engine.execute(lawId, outputName, parameters, calculationDate): ExecuteResult
engine.listLaws(): string[]
engine.getLawInfo(lawId): LawInfo
engine.hasLaw(lawId): boolean
engine.unloadLaw(lawId): boolean
engine.lawCount(): number
engine.version(): string
```

::: warning WASM Limitations
Cross-law references and open term resolution are not available in the WASM build. Pre-resolve dependencies in JavaScript and pass results as parameters.
:::

## Security Limits

The engine enforces compile-time security limits to prevent DoS:

| Limit | Value | Purpose |
|-------|-------|---------|
| `MAX_LOADED_LAWS` | 100 | Prevent memory exhaustion |
| `MAX_YAML_SIZE` | 1 MB | Prevent YAML bombs |
| `MAX_ARRAY_SIZE` | 1,000 | Prevent large array DoS |
| `MAX_RESOLUTION_DEPTH` | 50 | Internal reference nesting |
| `MAX_CROSS_LAW_DEPTH` | 20 | Cross-law reference nesting |
| `MAX_OPERATION_DEPTH` | 100 | Operation nesting |

## CLI Tools

```bash
# Execute a law
cargo run --bin evaluate -- \
    corpus/regulation/nl/wet/zorgtoeslagwet/2025-01-01.yaml \
    heeft_recht_op_zorgtoeslag \
    --param bsn 999993653 \
    --param vermogen 50000

# Validate a YAML file against schema
cargo run --bin validate --features validate -- \
    corpus/regulation/nl/wet/zorgtoeslagwet/2025-01-01.yaml
```

## Performance

Benchmarks are available via:

```bash
just bench
```

Key benchmarks: URI parsing, variable resolution, operations, article evaluation, law loading, priority resolution, and end-to-end service execution.

## Further Reading

- [Law Format](/concepts/law-format) - structure of law YAML files
- [RFC-003: Inversion of Control](/rfcs/rfc-003) - open terms and delegation
- [RFC-004: Uniform Operations](/rfcs/rfc-004) - operation syntax
- [RFC-007: Cross-Law Execution](/rfcs/rfc-007) - hooks, overrides, and temporal computation
