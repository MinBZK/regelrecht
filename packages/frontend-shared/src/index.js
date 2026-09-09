// @regelrecht/frontend-shared — shared frontend primitives consumed by the
// editor (frontend/, including its harvester "Beheer" section under
// frontend/src/harvester/) and lawmaking (frontend-lawmaking/). The editor is
// the canonical source; other apps conform to it.
export { apiFetch, apiFetchJson, apiFetchText, ApiError } from './apiFetch.js';
export { useAuth, ensureAuthReady, hasRole, hasAnyRole } from './useAuth.js';
export { useGithubAuth, ensureGithubReady } from './useGithubAuth.js';
// `applyColorScheme`, `createLocalStoragePersistence` and `VALID_THEMES` stay
// internal to useColorScheme — they're implementation details, not part of the
// package's public surface (no consumer imports them).
export { useColorScheme } from './useColorScheme.js';
// Engine value helpers (RFC-036): one definition of the Unknown value shape
// for the editor, the demo and the Gherkin runner.
export { isUnknown, missingFacts } from './values.js';
