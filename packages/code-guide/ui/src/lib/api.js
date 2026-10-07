// Thin client for `code-guide serve` (src/serve.rs).

async function getJson(url) {
  const res = await fetch(url);
  if (!res.ok) {
    const detail = (await res.text().catch(() => '')).trim();
    const err = new Error(detail || `${res.status} ${res.statusText}`);
    err.status = res.status;
    throw err;
  }
  return res.json();
}

const q = (params) => new URLSearchParams(params).toString();

/** Whether the index is fresh, stale or missing, and what the model counted. */
export const fetchStatus = () => getJson('api/status');

/** The crates, layered by the calls between them. */
export const fetchWorkspace = () => getJson('api/workspace');

/** One crate: its modules, layered by the calls between them. */
export const fetchCrate = (name) => getJson(`api/crate?${q({ name })}`);

/** One type: its methods with callers and callees, and its call graph. */
export const fetchType = ({ crate, module, name }) => getJson(`api/type?${q({ crate, module, name })}`);

/** One free function: its callers and callees. */
export const fetchFunction = ({ crate, module, name }) => getJson(`api/function?${q({ crate, module, name })}`);

/** A 1-based, inclusive line range of a Rust source file. */
export const fetchSource = ({ path, from, to }) =>
  getJson(`api/source?${q({ path, from: String(from), to: String(to) })}`);
