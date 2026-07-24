import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane, waitForSheet, fillSheetTextField, selectSheetDropdown, setSheetComboBox, openSheet, saveSheet } from './helpers.js';

test.describe('Inputs with sources', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'wet_op_de_zorgtoeslag', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);
  });

  test('add input with source reference', async ({ page }) => {
    await selectArticle(page, '2');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Add input: leeftijd from wet_basisregistratie_personen
    await page.locator('[data-testid="add-input-btn"]').click();
    await waitForSheet(page);

    await fillSheetTextField(page, 'Naam', 'leeftijd');
    await selectSheetDropdown(page, 'Type', 'number');
    // Bron regelgeving is a combo-box; bind the regulation id directly. With
    // no law-list / outputs mock, Bron output falls back to a plain text field.
    await setSheetComboBox(page, 'law-combo-box', 'wet_basisregistratie_personen');
    await page.waitForTimeout(100);
    const outputField = openSheet(page).locator('[data-testid="output-text-field"] input');
    await outputField.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'leeftijd');
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
