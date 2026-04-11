/**
 * Form-driven edit → re-execute loop end-to-end.
 *
 * Sister spec to `edit-test-loop.spec.js`. Same red → green flow on the
 * zorgtoeslag *Minderjarige* scenario, but performed entirely through the
 * structured editor on the right-hand Machine panel (EditSheet for inputs,
 * ActionSheet/OperationSettings for action operation trees) instead of the
 * middle-pane YAML textarea.
 *
 * The intent is to lock in the workflow the user actually wants to use:
 *  - "Nieuwe input" → fill EditSheet form (incl. source.parameters) → save
 *  - "Bewerk" on the heeft_recht_op_zorgtoeslag action → ActionSheet
 *  - Add a new operation under the AND → set type to GREATER_THAN_OR_EQUAL,
 *    subject $leeftijd, value 18 → save
 *  - Scenario badge flips to green
 *
 * Hand-editing YAML is the escape hatch; this is the happy path.
 */
import { test, expect } from '@playwright/test';
import { loadCorpus, loadScenario, mockCorpusApi } from './helpers-corpus.js';

/**
 * Wait for an ndd-sheet to be open. The host element itself doesn't change
 * `display`; visibility lives on the shadow-DOM `<dialog>`'s `open` attribute.
 */
async function waitForSheetOpen(page, hostSelector) {
  await page.waitForFunction((sel) => {
    const sheet = document.querySelector(sel);
    if (!sheet) return false;
    const dialog = sheet.shadowRoot?.querySelector('dialog');
    return dialog?.open === true;
  }, hostSelector, { timeout: 5000 });
}

/**
 * Wait for an ndd-sheet to be closed (dialog.open === false).
 */
async function waitForSheetClosed(page, hostSelector) {
  await page.waitForFunction((sel) => {
    const sheet = document.querySelector(sel);
    if (!sheet) return true;
    const dialog = sheet.shadowRoot?.querySelector('dialog');
    return dialog?.open !== true;
  }, hostSelector, { timeout: 5000 });
}

test.describe('Edit → re-execute loop via Machine panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      try {
        window.localStorage.removeItem('regelrecht-open-tabs');
      } catch { /* ignore */ }
    });
  });

  test('add leeftijd input + AND condition via Machine panel turns scenario green', async ({ page }) => {
    const corpus = loadCorpus();
    const zorgtoeslag = corpus.get('zorgtoeslagwet');
    expect(zorgtoeslag).toBeTruthy();

    const scenarioFilename = 'eligibility.feature';
    const scenarioText = loadScenario(zorgtoeslag.path, scenarioFilename);
    expect(scenarioText).toBeTruthy();

    await mockCorpusApi(
      page,
      corpus,
      { id: 'zorgtoeslagwet', scenarioFilename },
      scenarioText,
    );

    // Open article 2 directly via the URL param (avoids the
    // selectArticle helper bug noted in the previous PR).
    await page.goto('/editor.html?law=zorgtoeslagwet&article=2');
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 15_000 });

    // --- Initial state: Minderjarige scenario is red ---
    const minorHeader = page
      .locator('.sb-accordion-header')
      .filter({ hasText: 'Minderjarige' })
      .first();
    await expect(minorHeader).toBeVisible({ timeout: 30_000 });
    await minorHeader
      .locator('.sb-badge--pass, .sb-badge--fail')
      .first()
      .waitFor({ timeout: 30_000 });
    await expect(minorHeader).toHaveClass(/sb-header--fail/);

    // --- Toggle right pane to Machine view ---
    // EditorApp tags both segmented controls with data-testids so the
    // spec doesn't have to rely on positional `.nth()` selectors that
    // would silently break on layout reorder.
    await page.locator('[data-testid="right-pane-toggle"]').evaluate((el) => {
      el.value = 'machine';
      el.dispatchEvent(new Event('change', { bubbles: true }));
    });
    // Wait for the Machine panel to render the existing inputs section.
    await page.waitForSelector('[data-testid="machine-readable"]', { timeout: 5_000 });
    await page.waitForSelector('[data-testid="section-inputs"]', { timeout: 5_000 });

    // --- Click "Nieuwe input" to open EditSheet ---
    await page
      .locator('[data-testid="add-input-btn"]')
      .first()
      .evaluate((el) => el.click());

    const editSheet = page.locator('ndd-sheet.edit-sheet');
    await waitForSheetOpen(page, 'ndd-sheet.edit-sheet');

    // Helper: ndd-text-field renders an input inside its shadow DOM. The
    // light-DOM `input` element is reachable via the locator's `>> input`
    // descendant, but Vue listens to the host's `input` event. Set value
    // and dispatch the input event so Vue's @input handler updates the
    // model — same trick the existing helpers.fillSheetTextField uses.
    async function setSheetField(label, value) {
      const listItem = editSheet.locator('ndd-list-item').filter({
        hasText: label,
      });
      const input = listItem.locator('ndd-text-field input').first();
      await input.evaluate((el, val) => {
        el.value = val;
        el.dispatchEvent(new Event('input', { bubbles: true }));
      }, value);
    }

    async function setSheetDropdown(label, value) {
      const listItem = editSheet.locator('ndd-list-item').filter({
        hasText: label,
      });
      const select = listItem.locator('select').first();
      await select.evaluate((el, val) => {
        el.value = val;
        el.dispatchEvent(new Event('change', { bubbles: true }));
      }, value);
    }

    await setSheetField('Naam', 'leeftijd');
    await setSheetDropdown('Type', 'number');
    await setSheetField('Bron regelgeving', 'wet_basisregistratie_personen');
    await setSheetField('Bron output', 'leeftijd');

    // --- Add source.parameters: bsn=$bsn, peildatum=2025-01-01 ---
    const addParamBtn = editSheet.locator('[data-testid="source-param-add-btn"]');
    await addParamBtn.click();
    await addParamBtn.click();

    async function setParamRow(idx, key, value) {
      const keyInput = editSheet
        .locator(`[data-testid="source-param-key-${idx}"] input`)
        .first();
      const valInput = editSheet
        .locator(`[data-testid="source-param-value-${idx}"] input`)
        .first();
      await keyInput.evaluate((el, v) => {
        el.value = v;
        el.dispatchEvent(new Event('input', { bubbles: true }));
      }, key);
      await valInput.evaluate((el, v) => {
        el.value = v;
        el.dispatchEvent(new Event('input', { bubbles: true }));
      }, value);
    }
    await setParamRow(0, 'bsn', '$bsn');
    await setParamRow(1, 'peildatum', '2025-01-01');

    // --- Save the EditSheet ---
    // ndd-button renders its label via the `text` attribute through Lit's
    // shadow DOM and the property is not reflected back, so a [text=]
    // attribute selector or hasText filter wouldn't match the host. We
    // tagged the save button with a data-testid for stable targeting.
    await editSheet
      .locator('[data-testid="edit-sheet-save-btn"]')
      .evaluate((el) => el.click());
    await waitForSheetClosed(page, 'ndd-sheet.edit-sheet');

    // The new input should now appear in the Machine panel inputs list.
    await expect(
      page.locator('[data-testid="input-row-leeftijd"]'),
    ).toBeAttached({ timeout: 5_000 });

    // --- Open the heeft_recht_op_zorgtoeslag action via ActionSheet ---
    await page
      .locator('[data-testid="action-heeft_recht_op_zorgtoeslag-edit-btn"]')
      .evaluate((el) => el.click());

    const actionSheet = page.locator('ndd-sheet.action-sheet');
    await waitForSheetOpen(page, 'ndd-sheet.action-sheet');

    // The ActionSheet selects the LAST operation in the tree on open.
    // For the heeft_recht AND that's one of the leaf comparisons, not the
    // AND itself. We need to navigate up to the AND root (number "1") via
    // the "Bovenliggende operaties" list — each row carries a stable
    // data-testid keyed off the operation number.
    await actionSheet
      .locator('[data-testid="parent-op-1-edit-btn"]')
      .evaluate((el) => el.click());

    // The AND already has 3 conditions in zorgtoeslag article 2; each
    // surfaces as a `p.value-help-text` row with a "Bewerk" link. Snapshot
    // the count so we can assert the next click added a fourth row before
    // we try to interact with it (otherwise `.last()` could race against
    // Vue's flush and target the pre-existing third condition).
    const conditionLinks = actionSheet.locator('p.value-help-text a');
    const initialConditionCount = await conditionLinks.count();

    // Now OperationSettings shows the AND. Click "Voeg operatie toe"
    // which appends a fresh EQUALS condition to the AND's conditions[].
    await actionSheet
      .locator('[data-testid="add-nested-op-btn"]')
      .click();

    // Wait for the new condition's row to render before clicking it.
    await expect(conditionLinks).toHaveCount(initialConditionCount + 1);

    // The new condition appears as the last value row. Its "Bewerk" link
    // is inside the value-help-text paragraph; clicking it selects the
    // new operation in OperationSettings.
    await conditionLinks.last().click();

    // --- Configure the new condition ---
    // Change operation type to GREATER_THAN_OR_EQUAL via the dropdown.
    const opTypeDropdown = actionSheet.locator(
      '[data-testid="operation-type-dropdown"] select',
    );
    await opTypeDropdown.evaluate((el) => {
      el.value = 'GREATER_THAN_OR_EQUAL';
      el.dispatchEvent(new Event('change', { bubbles: true }));
    });

    // The OperationSettings now shows two rows: Onderwerp + Waarde.
    // Onderwerp is empty (a literal '') so it renders as a text input;
    // typing `$leeftijd` is interpreted as a variable ref by the engine.
    const onderwerpRow = actionSheet
      .locator('ndd-list-item')
      .filter({ hasText: 'Onderwerp' });
    await onderwerpRow
      .locator('ndd-text-field input')
      .first()
      .evaluate((el) => {
        el.value = '$leeftijd';
        el.dispatchEvent(new Event('input', { bubbles: true }));
      });

    const waardeRow = actionSheet
      .locator('ndd-list-item')
      .filter({ hasText: 'Waarde' });
    await waardeRow
      .locator('ndd-text-field input')
      .first()
      .evaluate((el) => {
        el.value = '18';
        el.dispatchEvent(new Event('input', { bubbles: true }));
      });

    // --- Save the ActionSheet ---
    await actionSheet
      .locator('[data-testid="action-sheet-save-btn"]')
      .evaluate((el) => el.click());
    await waitForSheetClosed(page, 'ndd-sheet.action-sheet');

    // --- Scenarios re-execute against the edited machine_readable ---
    // ScenarioBuilder is still mounted in the middle pane (we never
    // toggled it away), so the watch on `props.lawYaml` (= currentLawYaml)
    // fires when machineReadable mutates and re-runs all scenarios.
    await expect(minorHeader).toHaveClass(/sb-header--pass/, { timeout: 60_000 });
  });
});
