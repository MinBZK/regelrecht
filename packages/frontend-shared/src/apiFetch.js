/**
 * apiFetch — thin wrapper around `fetch` for the frontends' API calls (shared
 * by the editor and admin via @regelrecht/frontend-shared).
 *
 * It does exactly three things the ~50 hand-rolled call sites kept
 * re-implementing:
 *   1. ok-check: a non-ok response throws instead of flowing on silently;
 *   2. consistent error shaping: the thrown `ApiError` carries the HTTP
 *      `status`, the best-effort response `body` text and the response
 *      `contentType`, so callers can branch (`err.status === 404`) or
 *      surface the backend's own message;
 *   3. optional body parsing via `apiFetchJson` / `apiFetchText`.
 *
 * Deliberately NOT here: retries, timeouts, global state, auth handling
 * (the 401 redirect lives in `apiAuthGuard.js`, which wraps
 * `window.fetch` itself — calling the global `fetch` at call time keeps
 * that guard in the chain).
 *
 * `apiFetch` resolves with the raw `Response` so callers keep full
 * access to headers — the ETag/If-Match optimistic-concurrency plumbing
 * (useLaw, useScenarios, useTrajectDocuments) depends on that.
 *
 * Extra options on `init` (stripped before the real fetch):
 * - `allowStatuses: number[]` — non-ok statuses to resolve instead of
 *   throw, for callers that handle e.g. 404/412 as a normal branch.
 * - `errorMessage: string | (status, body, contentType) => string` —
 *   shapes the thrown error's message. Default: the response body when
 *   readable, else `HTTP <status>`. Call sites use this to keep their
 *   existing (Dutch) user-facing texts byte-for-byte identical.
 */

export class ApiError extends Error {
  /**
   * @param {string} message
   * @param {{ status: number, body?: string, contentType?: string }} info
   */
  constructor(message, { status, body = '', contentType = '' }) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.body = body;
    this.contentType = contentType;
  }
}

/** Best-effort body read: '' when the body is unreadable (network drop
 *  after headers, already-consumed stream, …). */
async function safeText(res) {
  try {
    return await res.text();
  } catch {
    return '';
  }
}

/**
 * `fetch` with an ok-check. Resolves with the raw `Response` (headers
 * stay accessible); throws `ApiError` on a non-ok status that is not in
 * `allowStatuses`. Network errors propagate unchanged, like plain fetch.
 *
 * @param {string} url
 * @param {RequestInit & { allowStatuses?: number[],
 *   errorMessage?: string |
 *     ((status: number, body: string, contentType: string) => string) }} init
 * @returns {Promise<Response>}
 */
export async function apiFetch(url, init = {}) {
  const { allowStatuses, errorMessage, ...fetchInit } = init;
  const res = await fetch(url, fetchInit);
  if (res.ok || (allowStatuses && allowStatuses.includes(res.status))) {
    return res;
  }
  const body = await safeText(res);
  // `?.` tolerates minimal Response-like stubs in tests; a real fetch
  // Response always has headers.
  const contentType = res.headers?.get('content-type') || '';
  const message =
    typeof errorMessage === 'function'
      ? errorMessage(res.status, body, contentType)
      : errorMessage || body || `HTTP ${res.status}`;
  throw new ApiError(message, { status: res.status, body, contentType });
}

/** `apiFetch` + `res.json()`. */
export async function apiFetchJson(url, init = {}) {
  const res = await apiFetch(url, init);
  return res.json();
}

/** `apiFetch` + `res.text()`. */
export async function apiFetchText(url, init = {}) {
  const res = await apiFetch(url, init);
  return res.text();
}
