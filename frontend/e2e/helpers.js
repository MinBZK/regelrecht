import { readFileSync } from 'fs';
import { resolve } from 'path';
import * as yaml from 'js-yaml';

const FIXTURE_DIR = resolve(import.meta.dirname, 'fixtures');

/**
 * A placeholder traject ref used across the e2e mocks. It satisfies the
 * router's `{slug}-{8hex}` shape (`[a-z0-9-]+-[0-9a-f]{8}`), so the
 * authenticated, traject-scoped editor route accepts it and the law API
 * hangs under `/api/trajects/${TEST_TRAJECT_REF}/corpus/...`.
 */
export const TEST_TRAJECT_REF = 'test-traject-0a1b2c3d';

// Fake, non-personal signed-in identity for the mocked `/auth/status`. No real
// names, emails, roles or tokens — just the shape `useAuth` reads. `.test` is a
// reserved TLD, so the address can never resolve to a real mailbox.
const TEST_PERSON = {
  sub: 'e2e-test-user',
  name: 'E2E Test',
  email: 'e2e@example.test',
  roles: [],
};

// Minimal traject the mocked `/api/trajects` list/detail returns. `ref` MUST
// match `TEST_TRAJECT_REF` so `useTrajects` resolves the active traject and
// `trajectMissing` stays false; the id is any UUID whose last 8 hex chars match
// the ref's suffix (the backend resolver keys off those).
const TEST_TRAJECT = {
  id: '00000000-0000-0000-0000-00000a1b2c3d',
  ref: TEST_TRAJECT_REF,
  name: 'E2E Test Traject',
  description: '',
  scope: '',
  status: 'bezig',
  role: 'owner',
};

/**
 * Load a YAML fixture file as a string.
 */
export function loadFixture(name) {
  return readFileSync(resolve(FIXTURE_DIR, name), 'utf-8');
}

/**
 * Mock the auth + traject endpoints so the `requiresAuth`, traject-scoped
 * editor routes mount under Playwright without a real backend or SSO. The
 * router guard awaits `/auth/status` (served as authenticated here), and
 * `useTrajects` needs the active ref present in the `/api/trajects` list.
 *
 * The traject detail handler only answers the bare `/api/trajects/{id}`
 * endpoint and falls through for its `/corpus/...` sub-routes, leaving those
 * to the corpus mocks.
 *
 * @param {import('@playwright/test').Page} page
 */
export async function mockAuthedEditor(page) {
  await page.route('**/auth/status', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        authenticated: true,
        oidc_configured: true,
        person: TEST_PERSON,
      }),
    }),
  );
  // GET /api/trajects - the membership list `useTrajects` reads.
  await page.route('**/api/trajects', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([TEST_TRAJECT]),
    }),
  );
  // GET /api/trajects/{id} - single traject detail. Only the bare detail path;
  // `/api/trajects/{ref}/corpus/...` falls through to the corpus handlers.
  await page.route('**/api/trajects/*', (route, request) => {
    const { pathname } = new URL(request.url());
    if (!/^\/api\/trajects\/[^/]+$/.test(pathname)) return route.fallback();
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ...TEST_TRAJECT,
        members: [],
        pending_invites: [],
        sources: [],
      }),
    });
  });
}

/**
 * Intercept the law API and serve a local YAML fixture instead. The glob
 * matches both the global (`/api/corpus/laws/{lawId}`) and the traject-scoped
 * (`/api/trajects/{ref}/corpus/laws/{lawId}`) endpoint, so the authenticated
 * editor - which reads through the active traject - is served the same fixture
 * as any legacy global caller.
 * @param {import('@playwright/test').Page} page
 * @param {string} lawId - e.g. 'wet_op_de_zorgtoeslag'
 * @param {string} fixtureName - e.g. 'zorgtoeslag-stripped.yaml'
 */
export async function interceptLaw(page, lawId, fixtureName) {
  const body = loadFixture(fixtureName);
  await page.route(`**/corpus/laws/${lawId}`, (route) =>
    route.fulfill({
      status: 200,
      contentType: 'text/yaml',
      body,
    }),
  );
  // Also intercept the default law id from the fixture itself
  if (lawId !== 'wet_op_de_zorgtoeslag') {
    await page.route('**/corpus/laws/wet_op_de_zorgtoeslag', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'text/yaml',
        body,
      }),
    );
  }
}

/**
 * Navigate to the traject-scoped editor and wait for it to load. Sets up the
 * auth + traject mocks first so the `requiresAuth` route mounts, then goes to
 * `/trajecten/${TEST_TRAJECT_REF}/editor/${lawId}[/${articleNumber}]`.
 * @param {import('@playwright/test').Page} page
 * @param {string} [lawId] - law id path segment
 * @param {string|number} [articleNumber] - optional article path segment
 */
export async function gotoEditor(page, lawId = 'wet_op_de_zorgtoeslag', articleNumber) {
  await mockAuthedEditor(page);
  const article = articleNumber != null ? `/${articleNumber}` : '';
  await page.goto(`/trajecten/${TEST_TRAJECT_REF}/editor/${lawId}${article}`);
  // Wait for the document tab bar to appear (articles loaded)
  await page.waitForSelector('nldd-document-tab-bar-item', { timeout: 10_000 });
}

/**
 * Make an article the active editing target.
 *
 * The editor's document tab bar only lists already-open (law, article) tabs,
 * and each item carries its label in the web component's `text` attribute
 * (shadow DOM), not as light-DOM text. So we match on that attribute. When the
 * requested article has no tab yet, we open it by pointing the traject-scoped
 * editor route at that article - exactly the state opening it from the document
 * list produces - then wait for and select its tab.
 * @param {import('@playwright/test').Page} page
 * @param {string|number} articleNumber
 */
export async function selectArticle(page, articleNumber) {
  const target = String(articleNumber);
  const tab = () =>
    page.locator(`nldd-document-tab-bar-item[text="Artikel ${target}"]`).first();
  if ((await tab().count()) === 0) {
    const { pathname, search } = new URL(page.url());
    // .../editor/{lawId}[/{article}] -> swap in (or append) the article segment.
    const next = pathname.replace(/(\/editor\/[^/]+)(?:\/[^/]+)?$/, `$1/${target}`);
    await page.goto(next + search);
    await tab().waitFor({ timeout: 10_000 });
  }
  await tab().click();
  // Small wait for reactivity to settle
  await page.waitForTimeout(200);
}

/**
 * The YAML pane's code editor. The editor moved from a plain
 * `.editor-yaml-textarea` to the design-system `nldd-code-editor` web
 * component, bound to the selected article's machine_readable YAML.
 * @param {import('@playwright/test').Page} page
 */
export function yamlEditor(page) {
  return page.locator('nldd-code-editor').first();
}

/**
 * Raw string content of the YAML code editor (the selected article's
 * machine_readable, serialised). Specs that string-manipulate the YAML read
 * this, edit it, and hand it back through {@link setYamlPane}.
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<string>}
 */
export async function readYamlSource(page) {
  return yamlEditor(page).evaluate((el) => el.value ?? '');
}

/**
 * Replace the YAML code editor content, driving the same input path a user's
 * keystrokes take. `nldd-code-editor` reads the new value from
 * `event.detail.value`, so we dispatch that shape directly - robust against
 * the component's internal editor state.
 * @param {import('@playwright/test').Page} page
 * @param {string} text
 */
export async function setYamlPane(page, text) {
  await yamlEditor(page).evaluate((el, val) => {
    el.value = val;
    el.dispatchEvent(new CustomEvent('input', { detail: { value: val }, bubbles: true }));
  }, text);
}

/**
 * Read the YAML pane content and parse it.
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<object|null>}
 */
export async function readYamlPane(page) {
  const text = await readYamlSource(page);
  if (!text.trim()) return null;
  return yaml.load(text);
}

/**
 * Get the machine_readable pane element.
 * @param {import('@playwright/test').Page} page
 */
export function machineReadablePane(page) {
  return page.locator('[data-testid="machine-readable"]');
}

/**
 * Click a button by its visible text within a container.
 * @param {import('@playwright/test').Page|import('@playwright/test').Locator} container
 * @param {string} text
 */
export async function clickButton(container, text) {
  await container.locator(`nldd-button:has-text("${text}")`).click();
}

/**
 * Fill an nldd-text-field by label within a container.
 * The nldd-text-field wraps a native <input> in shadow DOM.
 */
export async function fillTextField(container, label, value) {
  const listItem = container.locator(`nldd-list-item:has(nldd-text-cell:has-text("${label}"))`);
  const textField = listItem.locator('nldd-text-field');
  const input = textField.locator('input');
  await input.fill(value);
  await input.dispatchEvent('input');
}

/**
 * Select a value in an nldd-dropdown within a list item by label.
 */
export async function selectDropdown(container, label, value) {
  const listItem = container.locator(`nldd-list-item:has(nldd-text-cell:has-text("${label}"))`);
  const select = listItem.locator('nldd-dropdown select');
  await select.selectOption(value);
}

/**
 * The single open edit/action sheet. The editor keeps several `nldd-sheet`
 * hosts mounted at once (one per dialog kind), all but the open one hidden, so
 * a bare `nldd-sheet` locator is ambiguous. Filtering on visibility targets the
 * one dialog that is actually open.
 * @param {import('@playwright/test').Page} page
 */
export function openSheet(page) {
  return page.locator('nldd-sheet:visible');
}

/**
 * Wait for the edit sheet to be visible.
 * @param {import('@playwright/test').Page} page
 */
export async function waitForEditSheet(page) {
  await openSheet(page).first().waitFor({ state: 'visible', timeout: 5000 });
  await page.waitForTimeout(100);
}

/**
 * Click "Opslaan" in the edit sheet.
 * @param {import('@playwright/test').Page} page
 */
export async function saveEditSheet(page) {
  await openSheet(page).locator('nldd-button:has-text("Opslaan")').click();
  await page.waitForTimeout(200);
}

/**
 * Click "Opslaan" in the action sheet (nldd-sheet on main).
 * @param {import('@playwright/test').Page} page
 */
export async function saveActionSheet(page) {
  await openSheet(page).locator('nldd-button:has-text("Opslaan")').click();
  await page.waitForTimeout(200);
}

/**
 * Wait for an nldd-sheet dialog to be open (Lit component uses internal <dialog>).
 * @param {import('@playwright/test').Page} page
 */
export async function waitForSheet(page) {
  await page.waitForFunction(() => {
    return [...document.querySelectorAll('nldd-sheet')].some(
      (sheet) => sheet.shadowRoot?.querySelector('dialog')?.open ?? false,
    );
  }, { timeout: 5000 });
  await page.waitForTimeout(200);
}

/**
 * Fill an nldd-text-field input inside nldd-sheet by label text.
 * Uses evaluate to bypass shadow DOM visibility issues.
 */
export async function fillSheetTextField(page, labelText, value) {
  const sheet = openSheet(page);
  const listItem = sheet.locator(`nldd-list-item:has(nldd-text-cell:has-text("${labelText}"))`);
  const input = listItem.locator('nldd-text-field input');
  await input.evaluate((el, val) => {
    el.value = val;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

/**
 * Fill an nldd-number-field input inside nldd-sheet by label text.
 * Dispatches both native and custom events for Vue binding.
 */
export async function fillSheetNumberField(page, labelText, value) {
  const sheet = openSheet(page);
  const listItem = sheet.locator(`nldd-list-item:has(nldd-text-cell:has-text("${labelText}"))`);
  const numberField = listItem.locator('nldd-number-field');
  await numberField.evaluate((el, val) => {
    const input = el.shadowRoot?.querySelector('input') ?? el.querySelector('input');
    if (input) {
      input.value = String(val);
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    }
    el.dispatchEvent(new CustomEvent('change', { detail: { value: Number(val) }, bubbles: true }));
  }, value);
}

/**
 * Select a value in an nldd-dropdown inside nldd-sheet by label text.
 */
export async function selectSheetDropdown(page, labelText, value) {
  const sheet = openSheet(page);
  const listItem = sheet.locator(`nldd-list-item:has(nldd-text-cell:has-text("${labelText}"))`);
  const select = listItem.locator('select');
  await select.evaluate((el, val) => {
    el.value = val;
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
}

/**
 * Click "Opslaan" in the nldd-sheet.
 */
export async function saveSheet(page) {
  const btn = openSheet(page).locator('nldd-button:has-text("Opslaan")');
  await btn.evaluate(el => el.click());
  await page.waitForTimeout(300);
}
