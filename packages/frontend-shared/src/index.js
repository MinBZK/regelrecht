// @regelrecht/frontend-shared — shared frontend primitives consumed by the
// editor (frontend/), admin (packages/admin/frontend-src/) and lawmaking
// (frontend-lawmaking/). The editor is the canonical source; other apps
// conform to it.
export { apiFetch, apiFetchJson, apiFetchText, ApiError } from './apiFetch.js';
export { useAuth, ensureAuthReady } from './useAuth.js';
export {
  useColorScheme,
  applyColorScheme,
  createLocalStoragePersistence,
  VALID_THEMES,
} from './useColorScheme.js';
