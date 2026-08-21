// Helpers specific to the deployed-preview smoke suite. Selector helpers that
// the mocked `e2e/` suite already owns (article tabs, the machine-readable
// pane) are imported from `../e2e/helpers.js` rather than copied - see
// `smoke.spec.js`. What lives here is preview-only: the deterministic Keycloak
// login (ported from the `regelrecht-editor-login` skill) and the traject
// create/delete plumbing that drives setup/teardown through `page.request`.

/**
 * Deterministic Keycloak login for the shared test user, ported from the
 * `regelrecht-editor-login` skill. Navigates to `/auth/login`, fills the
 * username/password card (NOT "SSO Rijk"), submits, and waits until Keycloak
 * has bounced back to the app. Works both from a `Page` (global-setup) and is
 * side-effect only - the caller owns the resulting session cookie.
 *
 * @param {import('@playwright/test').Page} page
 * @param {object} opts
 * @param {string} opts.user
 * @param {string} opts.pass
 */
export async function login(page, { user, pass }) {
  if (!user || !pass) {
    throw new Error('login(): user and pass are required (set SMOKE_USER / SMOKE_PASS)');
  }
  // `/auth/login` is `requiresAuth` and boots the Keycloak redirect.
  await page.goto('/auth/login', { waitUntil: 'domcontentloaded' });

  // The username/password inputs live under a card that the accessibility
  // snapshot does not surface, but plain locators reach them fine.
  await page.waitForSelector('#username', { timeout: 30_000 });
  await page.fill('#username', user);
  await page.fill('#password', pass);

  // Submitting the password field posts the Keycloak form; wait until we've
  // left the login/Keycloak pages and are back on the app origin.
  await Promise.all([
    page
      .waitForURL(
        (url) => !url.href.includes('/auth/login') && !/keycloak|realms/.test(url.href),
        { timeout: 45_000 },
      )
      .catch(() => {}),
    page.press('#password', 'Enter'),
  ]);

  // Confirm the session is live before we hand back control. `/auth/status`
  // carries the just-minted session cookie automatically.
  await expectAuthenticated(page);
}

/**
 * Fetch `/auth/status` from within the page context and assert the session is
 * authenticated. Returns the parsed status object.
 * @param {import('@playwright/test').Page} page
 */
export async function expectAuthenticated(page) {
  const status = await page.evaluate(async () => {
    const res = await fetch('/auth/status', { headers: { accept: 'application/json' } });
    return res.json();
  });
  if (!status || status.authenticated !== true) {
    throw new Error(`Not authenticated after login: ${JSON.stringify(status)}`);
  }
  return status;
}

/**
 * Create a traject through the API with an ephemeral, timestamped name so a
 * re-run never collides with a leftover. Returns the created summary
 * (`{ id, ref, name, ... }`). The default (no custom repo) branch pushes to the
 * central corpus, which is what the smoke flow exercises.
 *
 * @param {import('@playwright/test').APIRequestContext} request
 * @param {object} [opts]
 * @param {string} [opts.prefix]
 */
export async function createTraject(request, { prefix = 'smoke' } = {}) {
  const name = `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const res = await request.post('/api/trajects', {
    headers: { 'content-type': 'application/json' },
    data: { name, description: 'Ephemeral traject aangemaakt door de smoke-suite.' },
  });
  if (!res.ok()) {
    throw new Error(`POST /api/trajects failed: ${res.status()} ${await res.text()}`);
  }
  const created = await res.json();
  if (!created.id || !created.ref) {
    throw new Error(`Unexpected traject create response: ${JSON.stringify(created)}`);
  }
  return created;
}

/**
 * Hard-delete a traject by UUID (owner-only, backend returns 204). Best-effort:
 * logs rather than throws so a teardown loop cleans up every traject even if
 * one delete fails.
 * @param {import('@playwright/test').APIRequestContext} request
 * @param {string} trajectId
 */
export async function deleteTraject(request, trajectId) {
  try {
    const res = await request.delete(`/api/trajects/${encodeURIComponent(trajectId)}`);
    if (!res.ok() && res.status() !== 404) {
      // eslint-disable-next-line no-console
      console.warn(`DELETE /api/trajects/${trajectId} -> ${res.status()} ${await res.text()}`);
    }
  } catch (err) {
    // eslint-disable-next-line no-console
    console.warn(`DELETE /api/trajects/${trajectId} threw: ${err.message}`);
  }
}

/**
 * Read one traject-scoped law body as text (YAML). Used to assert persistence
 * without re-parsing the DOM.
 * @param {import('@playwright/test').APIRequestContext} request
 * @param {string} trajectRef
 * @param {string} lawId
 */
export async function readTrajectLaw(request, trajectRef, lawId) {
  const res = await request.get(
    `/api/trajects/${encodeURIComponent(trajectRef)}/corpus/laws/${encodeURIComponent(lawId)}`,
    { headers: { accept: 'text/yaml' } },
  );
  if (!res.ok()) {
    throw new Error(`GET traject law failed: ${res.status()} ${await res.text()}`);
  }
  return res.text();
}
