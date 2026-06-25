// @regelrecht/frontend-shared — shared frontend primitives consumed by the
// editor (frontend/), admin (packages/admin/frontend-src/) and lawmaking
// (frontend-lawmaking/). The editor is the canonical source; other apps
// conform to it.
export { apiFetch, apiFetchJson, apiFetchText, ApiError } from './apiFetch.js';
export { useAuth, ensureAuthReady } from './useAuth.js';
// `applyColorScheme` and `createLocalStoragePersistence` stay internal to
// useColorScheme — they're implementation details, not part of the package's
// public surface.
export { useColorScheme, VALID_THEMES } from './useColorScheme.js';
