// apiFetch now lives in the shared package (@regelrecht/frontend-shared) so the
// editor, admin and lawmaking share one implementation. This thin re-export
// keeps the editor's ~25 call sites importing from './lib/apiFetch.js'
// unchanged. The 401 redirect still lives in ./apiAuthGuard.js (app-local).
export { apiFetch, apiFetchJson, apiFetchText, ApiError } from '@regelrecht/frontend-shared';
