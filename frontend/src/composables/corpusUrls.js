// Centralised URL builders for the editor's corpus endpoints. The two
// shapes - global (`/api/corpus/...`, read-only) and traject-scoped
// (`/api/trajects/{ref}/corpus/...`, read + write) - are picked by
// whether a traject ref is present at call time. Putting the choice in
// one place keeps composables aligned and prevents the pattern from
// drifting per file.
//
// The `trajectRef` is the URL-form identifier `{slug}-{8hex}` returned
// by the backend on `t.ref`. The backend resolves it back to a UUID via
// `resolve_traject_ref` - frontend code never needs to know the UUID
// shape, only the ref string.

function corpusBase(trajectRef) {
  return trajectRef
    ? `/api/trajects/${encodeURIComponent(trajectRef)}/corpus`
    : `/api/corpus`;
}

export function lawUrl(trajectRef, lawId) {
  return `${corpusBase(trajectRef)}/laws/${encodeURIComponent(lawId)}`;
}

// All versions of a law (newest-first), as a JSON array of YAML strings.
// Unlike `lawUrl` (one "best for today" body), the scenario dependency loader
// feeds every version to the engine so its date-aware version selection can
// pick the one in force on the scenario's calculation date - otherwise a
// referenced law that has a future-dated version would load only that future
// version and fail "not yet in force" for a past-dated scenario.
export function lawVersionsUrl(trajectRef, lawId) {
  return `${lawUrl(trajectRef, lawId)}/versions`;
}

export function lawsListUrl(trajectRef, query = '') {
  const q = query ? `?${query}` : '';
  return `${corpusBase(trajectRef)}/laws${q}`;
}

// Law ids edited in a traject (branch-vs-base diff). Only exists under the
// traject prefix - there is no global "changed laws" notion - so this is
// traject-only, like the documents builders. Callers already short-circuit
// the no-traject case (see `fetchChangedLawIds`); the guard here documents
// that contract and fails loudly if a future caller forgets it.
export function changedLawsUrl(trajectRef) {
  requireTraject(trajectRef, 'changed-laws listing');
  return `${corpusBase(trajectRef)}/changed-laws`;
}

// Law ids whose articles `implements` an open_term of `lawId` (the IoC
// reverse link). Computed server-side over the in-memory corpus, so the
// scenario dependency loader resolves implementing regulations with a
// single request instead of fetching and parsing every law in the corpus.
export function implementorsUrl(trajectRef, lawId) {
  return `${lawUrl(trajectRef, lawId)}/implementors`;
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

// Documents live under the traject-scope only. The list endpoint
// returns every document in the traject's documents folder, the file
// endpoint reads/writes one specific path. Both forms require a
// trajectRef - there is no global-scope counterpart by design.
export function documentsListUrl(trajectRef) {
  requireTraject(trajectRef, 'documents listing');
  return `${corpusBase(trajectRef)}/documents`;
}

// The document path is hierarchical (e.g. "mvt/concept.md"), so each
// segment is encoded individually instead of `encodeURIComponent`-ing
// the whole thing - that would turn `/` into `%2F` and break the
// axum wildcard match.
export function documentFileUrl(trajectRef, docPath) {
  requireTraject(trajectRef, 'document access');
  const encoded = docPath
    .split('/')
    .map(encodeURIComponent)
    .join('/');
  return `${corpusBase(trajectRef)}/documents/${encoded}`;
}

// Multipart upload of a PDF/Word document; the backend stores the bytes
// and enqueues an async conversion-to-markdown job. Traject-scoped only,
// like the other document builders.
export function documentUploadUrl(trajectRef) {
  requireTraject(trajectRef, 'document upload');
  return `${corpusBase(trajectRef)}/documents/upload`;
}

// Running/failed document-conversion jobs for the traject, backing the
// werkdocumenten conversion-status block.
export function documentJobsUrl(trajectRef) {
  requireTraject(trajectRef, 'document jobs listing');
  return `${corpusBase(trajectRef)}/documents/jobs`;
}

// A single conversion job — DELETE cancels it (kills a stuck upload).
export function documentJobUrl(trajectRef, jobId) {
  requireTraject(trajectRef, 'document job cancel');
  return `${corpusBase(trajectRef)}/documents/jobs/${encodeURIComponent(jobId)}`;
}

// Writes only exist under the traject prefix. Composables call this at
// the top of their save function so the call-stack failure is "no
// traject" instead of a malformed URL.
export function requireTraject(trajectRef, op = 'this operation') {
  if (!trajectRef) {
    throw new Error(`Cannot perform ${op} without an active traject`);
  }
}
