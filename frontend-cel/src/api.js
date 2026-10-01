// The routes of the runtime, of a cell and of a process. Every error comes
// back as {error: "..."}; the shared apiFetch does the ok check and throws an
// ApiError with that text as its message and the HTTP status as `status`.
import { apiFetch } from '@regelrecht/frontend-shared/apiFetch.js';

// The text under `error` in an error response, otherwise the HTTP status.
export function errorText(status, body) {
  try {
    const error = JSON.parse(body)?.error;
    if (typeof error === 'string' && error) return error;
  } catch {
    // Not JSON: then the status says it.
  }
  return `HTTP ${status}`;
}

async function request(method, path, body) {
  const resp = await apiFetch(path, {
    method,
    credentials: 'same-origin',
    headers: body ? { 'content-type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
    errorMessage: errorText,
  });
  return resp.status === 204 ? null : resp.json();
}

// The cells of the runtime, each with its chronicles and lexostatuses.
export const fetchCells = () => request('GET', '/api/cells');

// The processes of the runtime, each with its cell and possibilities.
export const fetchProcesses = () => request('GET', '/api/processes');

// What a cell recorded and what its reductions yield. The read routes of a
// cell are not open (the grams carry the identity of whoever submitted);
// a handler sees them through their process, under
// /processes/<process>/api/inspection/<cell>.
export function inspectionApi(process, cell) {
  const p = `/processes/${encodeURIComponent(process)}/api/inspection/${encodeURIComponent(cell)}`;
  return {
    chronicle: () => request('GET', `${p}/chronicle`),
    lexostatus: (name, input) =>
      request('GET', `${p}/lexostatus/${encodeURIComponent(name)}?${new URLSearchParams(input)}`),
  };
}

// The routes of a process, under /processes/<id>.
export function processApi(id) {
  const p = `/processes/${encodeURIComponent(id)}/api`;
  const actionPath = (root, name) => `${p}/cases/${encodeURIComponent(root)}/actions/${encodeURIComponent(name)}`;
  return {
    // Log in through a channel from `channels` in process.yaml: the fields of
    // the channel, and `role` when more than one role logs in through it.
    login: (channel, input) => request('POST', `${p}/channels/${encodeURIComponent(channel)}/login`, input),
    // Who is logged in, through whichever channel.
    session: () => request('GET', `${p}/session`),
    logout: (channel) => request('POST', `${p}/channels/${encodeURIComponent(channel)}/logout`),
    form: () => request('GET', `${p}/form`),
    assess: (external) => request('POST', `${p}/application/assessment`, { external }),
    submit: (external) => request('POST', `${p}/application`, { external }),
    possibilities: () => request('GET', `${p}/possibilities`),
    // Default data per action; also without a login.
    examples: () => request('GET', `${p}/examples`),
    // The YAML fragment behind a step of the "waarom?" (`path` from
    // fragmentPath in why.js); also without login.
    fragment: (path) => request('GET', `${p}${path}`),
    // The counter: an application that came in some other way,
    // {applicant, received_at, external}.
    submitAtCounter: (input) => request('POST', `${p}/counter/application`, input),
    worklist: () => request('GET', `${p}/worklist`),
    fetchCase: (root) => request('GET', `${p}/cases/${encodeURIComponent(root)}`),
    // An action in a case (the decision, a later stage, a fact from its
    // course): on trial, or taken and recorded. One route per action; which
    // ones there are, the case says.
    trialAction: (root, name, form) => request('POST', `${actionPath(root, name)}/trial`, { form }),
    // With `happened` the handler reports a fact that happened while the
    // trial said no on its content.
    takeAction: (root, name, form, happened = false) =>
      request('POST', actionPath(root, name), happened ? { form, happened } : { form }),
  };
}
