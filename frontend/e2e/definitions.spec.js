import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane, waitForSheet, fillSheetTextField, fillSheetNumberField, saveSheet } from './helpers.js';

test.describe('Definitions', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'zorgtoeslagwet', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);
  });

  test('add a numeric definition to article 1a', async ({ page }) => {
    await selectArticle(page, '1a');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Click "Nieuwe definitie" button
    await page.locator('ndd-button:has-text("Nieuwe definitie")').click();
    await waitForSheet(page);

    // Set name
    await fillSheetTextField(page, 'Naam', 'verantwoordelijke_autoriteit');

    // Set value (new definitions default to number)
    await fillSheetNumberField(page, 'Waarde', 42);

    // Save
    await saveSheet(page);

    // Verify the definition appears in the machine readable view
    const mrPane = page.locator('[data-testid="machine-readable"]');
    await expect(mrPane).toContainText('verantwoordelijke_autoriteit');

    // Verify YAML
    const yaml = await readYamlPane(page);
    expect(yaml.definitions).toHaveProperty('verantwoordelijke_autoriteit');
    // New definitions from EditSheet always store as {value: N} format
    expect(yaml.definitions.verantwoordelijke_autoriteit).toEqual({ value: 42 });
  });

  test('add numeric definitions to article 2', async ({ page }) => {
    await selectArticle(page, '2');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Add definition: drempelinkomen_alleenstaande = 3971900
    await page.locator('ndd-button:has-text("Nieuwe definitie")').click();
    await waitForSheet(page);

    await fillSheetTextField(page, 'Naam', 'drempelinkomen_alleenstaande');
    await fillSheetNumberField(page, 'Waarde', 3971900);
    await saveSheet(page);

    // Verify definition appears
    const mrPane = page.locator('[data-testid="machine-readable"]');
    await expect(mrPane).toContainText('drempelinkomen_alleenstaande');

    // Verify YAML
    const yaml = await readYamlPane(page);
    expect(yaml.definitions.drempelinkomen_alleenstaande).toEqual({ value: 3971900 });
  });
});
