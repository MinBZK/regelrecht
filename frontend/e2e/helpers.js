import { readFileSync } from 'fs';
import { resolve } from 'path';
import yaml from 'js-yaml';

const FIXTURE_DIR = resolve(import.meta.dirname, 'fixtures');

/**
 * Load a YAML fixture file as a string.
 */
export function loadFixture(name) {
  return readFileSync(resolve(FIXTURE_DIR, name), 'utf-8');
}

/**
 * Intercept the law API and serve a local YAML fixture instead.
 * @param {import('@playwright/test').Page} page
 * @param {string} lawId - e.g. 'wet_op_de_zorgtoeslag' or 'zorgtoeslagwet'
 * @param {string} fixtureName - e.g. 'zorgtoeslag-stripped.yaml'
 */
export async function interceptLaw(page, lawId, fixtureName) {
  const body = loadFixture(fixtureName);
  await page.route(`**/api/corpus/laws/${lawId}`, (route) =>
    route.fulfill({
      status: 200,
      contentType: 'text/yaml',
      body,
    }),
  );
  // Also intercept the default law id from the fixture itself
  if (lawId !== 'zorgtoeslagwet') {
    await page.route('**/api/corpus/laws/zorgtoeslagwet', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'text/yaml',
        body,
      }),
    );
  }
}

/**
 * Navigate to the editor and wait for it to load.
 * @param {import('@playwright/test').Page} page
 * @param {string} [lawId] - law query param
 */
export async function gotoEditor(page, lawId = 'zorgtoeslagwet') {
  await page.goto(`/editor/${lawId}`);
  // Wait for the document tab bar to appear (articles loaded)
  await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 10_000 });
}

/**
 * Click an article tab in the editor.
 * @param {import('@playwright/test').Page} page
 * @param {string|number} articleNumber
 */
export async function selectArticle(page, articleNumber) {
  const tabs = page.locator('ndd-document-tab-bar-item');
  const count = await tabs.count();
  for (let i = 0; i < count; i++) {
    const text = await tabs.nth(i).textContent();
    if (text.trim().includes(`Artikel ${articleNumber}`)) {
      await tabs.nth(i).click();
      // Small wait for reactivity to settle
      await page.waitForTimeout(200);
      return;
    }
  }
  throw new Error(`Article ${articleNumber} tab not found`);
}

/**
 * Read the YAML textarea content and parse it.
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<object|null>}
 */
export async function readYamlPane(page) {
  const textarea = page.locator('.editor-yaml-textarea');
  const text = await textarea.inputValue();
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
  await container.locator(`ndd-button:has-text("${text}")`).click();
}

/**
 * Fill an ndd-text-field by label within a container.
 * The ndd-text-field wraps a native <input> in shadow DOM.
 */
export async function fillTextField(container, label, value) {
  const listItem = container.locator(`ndd-list-item:has(ndd-text-cell:has-text("${label}"))`);
  const textField = listItem.locator('ndd-text-field');
  const input = textField.locator('input');
  await input.fill(value);
  await input.dispatchEvent('input');
}

/**
 * Select a value in an ndd-dropdown within a list item by label.
 */
export async function selectDropdown(container, label, value) {
  const listItem = container.locator(`ndd-list-item:has(ndd-text-cell:has-text("${label}"))`);
  const select = listItem.locator('ndd-dropdown select');
  await select.selectOption(value);
}

/**
 * Wait for the edit sheet to be visible.
 * @param {import('@playwright/test').Page} page
 */
export async function waitForEditSheet(page) {
  await page.locator('ndd-sheet').waitFor({ state: 'visible', timeout: 5000 });
  await page.waitForTimeout(100);
}

/**
 * Click "Opslaan" in the edit sheet.
 * @param {import('@playwright/test').Page} page
 */
export async function saveEditSheet(page) {
  const sheet = page.locator('ndd-sheet');
  await sheet.locator('ndd-button:has-text("Opslaan")').click();
  await page.waitForTimeout(200);
}

/**
 * Click "Opslaan" in the action sheet (ndd-sheet on main).
 * @param {import('@playwright/test').Page} page
 */
export async function saveActionSheet(page) {
  const sheet = page.locator('ndd-sheet');
  await sheet.locator('ndd-button:has-text("Opslaan")').click();
  await page.waitForTimeout(200);
}

/**
 * Wait for the ndd-sheet dialog to be open (Lit component uses internal <dialog>).
 * @param {import('@playwright/test').Page} page
 */
export async function waitForSheet(page) {
  await page.waitForFunction(() => {
    const sheet = document.querySelector('ndd-sheet');
    if (!sheet) return false;
    const dialog = sheet.shadowRoot?.querySelector('dialog');
    return dialog?.open ?? false;
  }, { timeout: 5000 });
  await page.waitForTimeout(200);
}

/**
 * Fill an ndd-text-field input inside ndd-sheet by label text.
 * Uses evaluate to bypass shadow DOM visibility issues.
 */
export async function fillSheetTextField(page, labelText, value) {
  const sheet = page.locator('ndd-sheet');
  const listItem = sheet.locator(`ndd-list-item:has(ndd-text-cell:has-text("${labelText}"))`);
  const input = listItem.locator('ndd-text-field input');
  await input.evaluate((el, val) => {
    el.value = val;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

/**
 * Fill an ndd-number-field input inside ndd-sheet by label text.
 * Dispatches both native and custom events for Vue binding.
 */
export async function fillSheetNumberField(page, labelText, value) {
  const sheet = page.locator('ndd-sheet');
  const listItem = sheet.locator(`ndd-list-item:has(ndd-text-cell:has-text("${labelText}"))`);
  const numberField = listItem.locator('ndd-number-field');
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
 * Select a value in an ndd-dropdown inside ndd-sheet by label text.
 */
export async function selectSheetDropdown(page, labelText, value) {
  const sheet = page.locator('ndd-sheet');
  const listItem = sheet.locator(`ndd-list-item:has(ndd-text-cell:has-text("${labelText}"))`);
  const select = listItem.locator('select');
  await select.evaluate((el, val) => {
    el.value = val;
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
}

/**
 * Click "Opslaan" in the ndd-sheet.
 */
export async function saveSheet(page) {
  const sheet = page.locator('ndd-sheet');
  const btn = sheet.locator('ndd-button:has-text("Opslaan")');
  await btn.evaluate(el => el.click());
  await page.waitForTimeout(300);
}
