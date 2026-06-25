// Auth state (`useAuth`) now lives in the shared package
// (@regelrecht/frontend-shared); admin conforms to the editor's implementation.
//
// `authedFetch` stays admin-local glue: the editor installs a global
// window.fetch 401 guard (apiAuthGuard.js), but admin has none and instead
// redirects inline and returns `null` on 401 so call sites can
// `if (!response) return;`. It deliberately keeps bare-`fetch` semantics
// (non-throwing; the raw Response is returned for every non-401 status) because
// the call sites inspect `response.status`/`response.ok` themselves.
import { useAuth } from '@regelrecht/frontend-shared';

export { useAuth };

// One shared `login` closure; with no argument the return_url defaults to the
// current location (same behavior as the old admin-local redirectToLogin).
const { login } = useAuth();

/**
 * Like fetch(), but redirects to the login page on 401 and returns `null`
 * so callers can `return` early. For other statuses the Response is
 * returned unchanged — call sites still own 4xx/5xx body handling.
 */
export async function authedFetch(input, init) {
  const response = await fetch(input, init);
  if (response.status === 401) {
    login();
    return null;
  }
  return response;
}
