import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane, waitForSheet, fillSheetTextField, selectSheetDropdown, saveSheet } from './helpers.js';

test.describe('Inputs with sources', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'zorgtoeslagwet', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);
  });

  test('add input with source reference', async ({ page }) => {
    await selectArticle(page, '2');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Add input: leeftijd from wet_basisregistratie_personen
    await page.locator('nldd-button:has-text("Nieuwe input")').click();
    await waitForSheet(page);

    await fillSheetTextField(page, 'Naam', 'leeftijd');
    await selectSheetDropdown(page, 'Type', 'number');
    await fillSheetTextField(page, 'Bron regelgeving', 'wet_basisregistratie_personen');
    await fillSheetTextField(page, 'Bron output', 'leeftijd');
    await saveSheet(page);

    // Verify YAML
    const yaml = await readYamlPane(page);
    expect(yaml.execution.input).toHaveLength(1);
    expect(yaml.execution.input[0].name).toBe('leeftijd');
    expect(yaml.execution.input[0].type).toBe('number');
    expect(yaml.execution.input[0].source.regulation).toBe('wet_basisregistratie_personen');
    expect(yaml.execution.input[0].source.output).toBe('leeftijd');
  });
});
