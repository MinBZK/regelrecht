// @regelrecht/frontend-shared — shared frontend primitives consumed by the
// editor (frontend/, including its harvester "Beheer" section under
// frontend/src/harvester/), lawmaking (frontend-lawmaking/), the demo
// (frontend-demo/) and the OCW-pocs (frontend-poc-*). The editor is the
// canonical source for what lives here; other apps conform to it.
//
// Niet alles hier hangt aan deze index. De Gherkin-runner, de losse
// lib-modules en de gedeelde componenten hebben hun eigen subpad in
// `exports` (./gherkin, ./lib/*, ./components/*, ./useBewaardeStand.js),
// zodat een app alleen binnenhaalt wat hij gebruikt.
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
