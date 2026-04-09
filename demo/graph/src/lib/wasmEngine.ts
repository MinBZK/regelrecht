/**
 * wasmEngine — singleton WasmEngine loader for the demo graph.
 *
 * Ported from frontend/src/composables/useEngine.js. Uses a fetch+blob URL
 * pattern to load the JS glue so Vite does not try to resolve /static assets
 * as source modules. The wasm pkg lives under static/wasm/pkg/ (symlinked to
 * frontend/public/wasm/pkg/ in dev, copied in prod build).
 */

// --- Minimal hand-typed interfaces matching packages/engine/src/wasm.rs ---

export type Value = null | boolean | number | string | Value[] | { [k: string]: Value };

export interface PathNode {
  node_type: string; // snake_case: resolve | operation | action | requirement
  //                    | cross_law_reference | article | cached
  //                    | open_term_resolution | hook_resolution | override_resolution
  name: string;
  result?: Value;
  resolve_type?: string; // SCREAMING_SNAKE_CASE: PARAMETER | OUTPUT | ...
  children?: PathNode[];
  duration_us?: number;
  message?: string;
}

export interface OutputProvenance {
  type: 'Direct' | 'Reactive' | 'Override' | string;
  law_id?: string;
  article?: string;
  hook_point?: string;
}

export interface TraceResult {
  outputs: Record<string, Value>;
  output_provenance?: Record<string, OutputProvenance>;
  resolved_inputs: Record<string, Value>;
  article_number: string;
  law_id: string;
  law_uuid?: string;
  trace?: PathNode;
  trace_text?: string;
  engine_version: string;
  schema_version?: string;
  regulation_hash?: string;
  regulation_valid_from?: string;
}

export interface WasmEngine {
  loadLaw(yaml: string): string;
  execute(
    law_id: string,
    output_name: string,
    parameters: Record<string, unknown>,
    calculation_date: string,
  ): TraceResult;
  executeWithTrace(
    law_id: string,
    output_name: string,
    parameters: Record<string, unknown>,
    calculation_date: string,
  ): TraceResult;
  hasLaw(law_id: string): boolean;
  listLaws(): string[];
  unloadLaw(law_id: string): boolean;
  lawCount(): number;
  clearDataSources(): void;
  registerDataSource(
    name: string,
    key_field: string,
    records: Record<string, unknown>[],
  ): void;
  version(): string;
}

let enginePromise: Promise<WasmEngine> | null = null;

export async function initEngine(): Promise<WasmEngine> {
  if (enginePromise) return enginePromise;

  enginePromise = (async () => {
    const jsRes = await fetch('/wasm/pkg/regelrecht_engine.js');
    if (!jsRes.ok) {
      throw new Error(
        `Failed to fetch WASM JS glue (${jsRes.status}). Did you run \`just wasm-build\`?`,
      );
    }
    const jsText = await jsRes.text();
    const blob = new Blob([jsText], { type: 'application/javascript' });
    const blobUrl = URL.createObjectURL(blob);
    const wasm = await import(/* @vite-ignore */ blobUrl);
    URL.revokeObjectURL(blobUrl);
    await wasm.default('/wasm/pkg/regelrecht_engine_bg.wasm');
    return new wasm.WasmEngine() as WasmEngine;
  })();

  return enginePromise;
}
