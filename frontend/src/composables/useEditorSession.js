/**
 * Editor session id — minted once per browser tab, used to scope the
 * federated write-back path's per-(session, source) feature branch + PR.
 *
 * Why sessionStorage and not localStorage:
 * - Closing the tab (or the browser) purges the id, so the next visit
 *   gets a fresh PR. That matches the "one PR per editor session"
 *   contract the backend enforces — no separate server-side TTL needed.
 * - Two tabs on the same browser get two ids, hence two parallel PRs.
 *   That is intentional: independent edit streams shouldn't collide on
 *   one branch.
 *
 * The id is sent on every write request as `X-Editor-Session`. The backend
 * uses it (alongside the law's source id) to look up or lazily build a
 * `SessionGitBackend` that pushes to `editor/session-<id>` and ensures a
 * PR upstream.
 *
 * Format: a UUID without dashes-prefix, just to keep the resulting branch
 * name short and human-readable when it shows up on GitHub.
 */

import { ref } from 'vue';

const STORAGE_KEY = 'regelrecht-editor-session-id';

/**
 * Module-level shared ref so every composable that performs a save
 * (useLaw, useScenarios, …) updates the same value — and EditorApp's
 * "Bekijk op GitHub" badge picks up the most recent PR regardless of
 * which pane triggered the save. Mirrors the "one PR per editor session"
 * contract: every save in the same browser tab lands on the same upstream
 * branch, so a single shared ref is exactly what the badge needs.
 */
export const lastSavedPr = ref(null);

/**
 * Returns this tab's editor session id, minting + persisting one on first
 * call. Stable for the lifetime of the tab.
 *
 * Falls back to an in-memory id when sessionStorage is unavailable
 * (e.g. private-mode quirks): saves still work, but the user gets a new
 * PR each page load. That's degraded behaviour, not a hard failure.
 */
export function getEditorSessionId() {
  let id = readFromStorage();
  if (id) return id;
  id = mintSessionId();
  writeToStorage(id);
  return id;
}

function readFromStorage() {
  try {
    return window.sessionStorage.getItem(STORAGE_KEY) || null;
  } catch {
    return null;
  }
}

function writeToStorage(id) {
  try {
    window.sessionStorage.setItem(STORAGE_KEY, id);
  } catch {
    // ignore — we'll fall back to in-memory minting on the next call
  }
}

/**
 * Mint a new id. Uses the platform `crypto.randomUUID()` when available
 * (modern browsers, http*s* origins) and falls back to a math-based
 * generator that's not cryptographically random but is good enough as
 * an opaque key — this id is not a secret, it just needs to be unique.
 */
function mintSessionId() {
  const cryptoApi = typeof crypto !== 'undefined' ? crypto : null;
  if (cryptoApi && typeof cryptoApi.randomUUID === 'function') {
    return cryptoApi.randomUUID().replace(/-/g, '');
  }
  // Fallback: 16 random bytes hex-encoded.
  const bytes = new Uint8Array(16);
  if (cryptoApi && typeof cryptoApi.getRandomValues === 'function') {
    cryptoApi.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(Math.random() * 256);
  }
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}
