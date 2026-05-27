// Centralised URL builders for the editor's corpus endpoints. The two
// shapes — global (`/api/corpus/...`, read-only) and traject-scoped
// (`/api/trajects/{ref}/corpus/...`, read + write) — are picked by
// whether a traject ref is present at call time. Putting the choice in
// one place keeps composables aligned and prevents the pattern from
// drifting per file.
//
// The `trajectRef` is the URL-form identifier `{slug}-{8hex}` returned
// by the backend on `t.ref`. The backend resolves it back to a UUID via
// `resolve_traject_ref` — frontend code never needs to know the UUID
// shape, only the ref string.

function corpusBase(trajectRef) {
  return trajectRef
    ? `/api/trajects/${encodeURIComponent(trajectRef)}/corpus`
    : `/api/corpus`;
}

export function lawUrl(trajectRef, lawId) {
  return `${corpusBase(trajectRef)}/laws/${encodeURIComponent(lawId)}`;
}

export function lawsListUrl(trajectRef, query = '') {
  const q = query ? `?${query}` : '';
  return `${corpusBase(trajectRef)}/laws${q}`;
}

export function scenariosListUrl(trajectRef, lawId) {
  return `${lawUrl(trajectRef, lawId)}/scenarios`;
}

export function scenarioFileUrl(trajectRef, lawId, filename) {
  return `${scenariosListUrl(trajectRef, lawId)}/${encodeURIComponent(filename)}`;
}

export function annotationsUrl(trajectRef, lawId) {
  return `${lawUrl(trajectRef, lawId)}/annotations`;
}

// Writes only exist under the traject prefix. Composables call this at
// the top of their save function so the call-stack failure is "no
// traject" instead of a malformed URL.
export function requireTraject(trajectRef, op = 'this operation') {
  if (!trajectRef) {
    throw new Error(`Cannot perform ${op} without an active traject`);
  }
}
