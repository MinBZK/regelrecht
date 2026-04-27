import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane, waitForSheet, fillSheetTextField, selectSheetDropdown, saveSheet } from './helpers.js';

test.describe('Parameters and Outputs', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'zorgtoeslagwet', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);
  });

  test('add parameter and outputs to article 2', async ({ page }) => {
    await selectArticle(page, '2');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // --- Add parameter: bsn (string, required) ---
    await page.locator('nldd-button:has-text("Nieuwe parameter")').click();
    await waitForSheet(page);

    await fillSheetTextField(page, 'Naam', 'bsn');

    // Toggle required
    const sheet = page.locator('nldd-sheet');
    const requiredSwitch = sheet.locator('nldd-switch');
    await requiredSwitch.evaluate(el => el.click());
    await page.waitForTimeout(100);

    await saveSheet(page);

    // Verify parameter appears
    await expect(page.locator('[data-testid="machine-readable"]')).toContainText('bsn');

    // --- Add output: heeft_recht_op_zorgtoeslag (boolean) ---
    await page.locator('nldd-button:has-text("Nieuwe output")').click();
    await waitForSheet(page);

    await fillSheetTextField(page, 'Naam', 'heeft_recht_op_zorgtoeslag');
    await selectSheetDropdown(page, 'Type', 'boolean');
    await saveSheet(page);

    // --- Add output: hoogte_zorgtoeslag (amount) ---
    await page.locator('nldd-button:has-text("Nieuwe output")').click();
    await waitForSheet(page);

    await fillSheetTextField(page, 'Naam', 'hoogte_zorgtoeslag');
    await selectSheetDropdown(page, 'Type', 'amount');
    await saveSheet(page);

    // Verify YAML
    const yaml = await readYamlPane(page);
    expect(yaml.execution.parameters).toHaveLength(1);
    expect(yaml.execution.parameters[0].name).toBe('bsn');
    expect(yaml.execution.parameters[0].type).toBe('string');
    expect(yaml.execution.output).toHaveLength(2);
    expect(yaml.execution.output[0].name).toBe('heeft_recht_op_zorgtoeslag');
    expect(yaml.execution.output[0].type).toBe('boolean');
    expect(yaml.execution.output[1].name).toBe('hoogte_zorgtoeslag');
    expect(yaml.execution.output[1].type).toBe('amount');
  });
});
