/**
 * Datatype-driven scenario parameter inputs.
 *
 * Verifies that a scenario parameter renders the control matching its
 * declared datatype (ScenarioParameterInput): a boolean input becomes a
 * switch, an amount/eurocent input a number-field showing euros, and a
 * string a plain text-field. Drives the real editor with a mocked corpus
 * (no editor-api / Postgres needed) and opens the scenario edit sheet.
 */
import { test, expect } from '@playwright/test';
import { loadFixture } from './helpers.js';
import { mockCorpusApi } from './helpers-corpus.js';

// A minimal scenario that passes one parameter of each declared datatype:
// bsn (string), is_verzekerde (boolean input), toetsingsinkomen (amount,
// unit eurocent). Execution may not fully succeed without cross-law data,
// but the scenario card and its edit sheet still render, which is what we
// assert against.
const SCENARIO = `Feature: Typed input controls

  Scenario: Mixed parameter types
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "is_verzekerde" is "true"
    Given parameter "toetsingsinkomen" is 150000
    When I evaluate "heeft_recht_op_zorgtoeslag" of "zorgtoeslagwet"
    Then the execution succeeds
`;

test.describe('Scenario parameter input controls', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      try { window.localStorage.removeItem('regelrecht-open-tabs'); } catch { /* ignore */ }
    });

    const fixture = loadFixture('zorgtoeslag-full.yaml');
    const corpus = new Map([
      ['zorgtoeslagwet', { content: fixture, path: '', pubDate: '2024-12-20' }],
    ]);
    await mockCorpusApi(
      page,
      corpus,
      { id: 'zorgtoeslagwet', scenarioFilename: 'typed.feature' },
      SCENARIO,
    );
  });

  test('renders a switch for boolean, number-field for amount, text-field for string', async ({ page }) => {
    await page.goto('/editor/zorgtoeslagwet/2');
    await page.waitForSelector('nldd-document-tab-bar-item', { timeout: 15_000 });

    // Open the scenario edit sheet via the card's "Bewerk" button.
    const edit = page.getByRole('button', { name: 'Bewerk' }).first();
    await edit.waitFor({ timeout: 30_000 });
    await edit.click();

    // Each parameter renders as a list-item: the name plus the typed control.
    // Scope by the list-item carrying the parameter name. The sheet opening
    // is implied by the controls becoming visible below.
    const itemFor = (name) => page.locator('nldd-list-item').filter({ hasText: name });

    // boolean -> switch (and the stored "true" reflects as checked)
    const sw = itemFor('is_verzekerde').locator('nldd-switch-field');
    await expect(sw).toBeVisible({ timeout: 10_000 });
    expect(await sw.evaluate((el) => el.checked)).toBe(true);

    // amount/eurocent -> number-field showing euros (150000 eurocent -> 1500)
    const amount = itemFor('toetsingsinkomen').locator('nldd-number-field');
    await expect(amount).toBeVisible();
    expect(await amount.evaluate((el) => el.value)).toBe(1500);

    // string -> plain text-field
    await expect(itemFor('bsn').locator('nldd-text-field')).toBeVisible();
  });
});
