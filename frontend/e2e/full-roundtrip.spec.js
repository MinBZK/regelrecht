import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane } from './helpers.js';
import yaml from 'js-yaml';
import { readFileSync } from 'fs';
import { resolve } from 'path';

/**
 * Load the original (with machine_readable) zorgtoeslag YAML.
 */
function loadOriginal() {
  const path = resolve(import.meta.dirname, '../../corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml');
  return yaml.load(readFileSync(path, 'utf-8'));
}

test.describe('Full round-trip', () => {
  test('YAML textarea round-trips without data loss', async ({ page }) => {
    // Load the FULL zorgtoeslag (with machine_readable) instead of stripped
    const original = loadOriginal();
    const fullYaml = readFileSync(
      resolve(import.meta.dirname, '../../corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml'),
      'utf-8'
    );

    await page.route('**/api/corpus/laws/zorgtoeslagwet', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fullYaml })
    );
    await page.goto('/editor.html?law=zorgtoeslagwet');
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 10_000 });

    // Article 1a: simple definition
    await selectArticle(page, '1a');
    await page.waitForTimeout(300);
    const yaml1a = await readYamlPane(page);
    expect(yaml1a.definitions.verantwoordelijke_autoriteit).toBe(
      original.articles[1].machine_readable.definitions.verantwoordelijke_autoriteit
    );

    // Article 2: complex structure with definitions, execution, actions
    await selectArticle(page, '2');
    await page.waitForTimeout(300);
    const yaml2 = await readYamlPane(page);

    // Verify definitions
    const origDefs = original.articles[2].machine_readable.definitions;
    expect(yaml2.definitions.drempelinkomen_alleenstaande).toEqual(origDefs.drempelinkomen_alleenstaande);
    expect(yaml2.definitions.percentage_toetsingsinkomen).toEqual(origDefs.percentage_toetsingsinkomen);

    // Verify execution structure
    expect(yaml2.execution.produces.legal_character).toBe('BESCHIKKING');
    expect(yaml2.execution.produces.decision_type).toBe('TOEKENNING');
    expect(yaml2.execution.parameters[0].name).toBe('bsn');
    const origExec = original.articles[2].machine_readable.execution;
    expect(yaml2.execution.input).toHaveLength(origExec.input.length);
    expect(yaml2.execution.output).toHaveLength(origExec.output.length);
    expect(yaml2.execution.actions).toHaveLength(origExec.actions.length);

    // Verify action structure (hoogte_zorgtoeslag - MAX operation)
    const hoogte = yaml2.execution.actions[0];
    expect(hoogte.output).toBe('hoogte_zorgtoeslag');
    expect(hoogte.value.operation).toBe('MAX');
    expect(hoogte.value.values).toHaveLength(2);
    expect(hoogte.value.values[0]).toBe(0);
    expect(hoogte.value.values[1].operation).toBe('SUBTRACT');

    // Verify action structure (heeft_recht - AND operation)
    const heeftRecht = yaml2.execution.actions[1];
    expect(heeftRecht.output).toBe('heeft_recht_op_zorgtoeslag');
    expect(heeftRecht.value.operation).toBe('AND');
    expect(heeftRecht.value.conditions).toHaveLength(origExec.actions[1].value.conditions.length);

    // Article 3: vermogen_onder_grens
    await selectArticle(page, '3');
    await page.waitForTimeout(300);
    const yaml3 = await readYamlPane(page);
    expect(yaml3.execution.actions[0].output).toBe('vermogen_onder_grens');
    expect(yaml3.execution.actions[0].value.operation).toBe('LESS_THAN_OR_EQUAL');

    // Article 5: simple action
    await selectArticle(page, '5');
    await page.waitForTimeout(300);
    const yaml5 = await readYamlPane(page);
    expect(yaml5.execution.actions[0].output).toBe('bevoegd_gezag');
    expect(yaml5.execution.actions[0].value).toBe('Dienst Toeslagen');

    // Article 8: simple action
    await selectArticle(page, '8');
    await page.waitForTimeout(300);
    const yaml8 = await readYamlPane(page);
    expect(yaml8.execution.actions[0].output).toBe('wet_naam');
    expect(yaml8.execution.actions[0].value).toBe('Wet op de zorgtoeslag');
  });

  test('YAML editing creates valid structure that displays correctly', async ({ page }) => {
    await interceptLaw(page, 'zorgtoeslagwet', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);

    await selectArticle(page, '2');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Edit YAML directly to add complete machine_readable
    const textarea = page.locator('.editor-yaml-textarea');
    const mrYaml = `definitions:
  drempelinkomen_alleenstaande:
    value: 3971900
  percentage_toetsingsinkomen:
    value: 0.137
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: TOEKENNING
  parameters:
    - name: bsn
      type: string
      required: true
  input:
    - name: leeftijd
      type: number
      source:
        regulation: wet_basisregistratie_personen
        output: leeftijd
  output:
    - name: heeft_recht_op_zorgtoeslag
      type: boolean
  actions:
    - output: heeft_recht_op_zorgtoeslag
      value:
        operation: GREATER_THAN_OR_EQUAL
        subject: $leeftijd
        value: 18
`;

    await textarea.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, mrYaml);
    await page.waitForTimeout(300);

    // Verify the machine readable view shows the data
    const mrPane = page.locator('[data-testid="machine-readable"]');
    await expect(mrPane).toContainText('drempelinkomen_alleenstaande');
    await expect(mrPane).toContainText('BESCHIKKING');
    await expect(mrPane).toContainText('bsn');
    await expect(mrPane).toContainText('leeftijd');
    await expect(mrPane).toContainText('heeft_recht_op_zorgtoeslag');

    // Verify the action is displayed and can be opened
    const actionItem = page.locator('ndd-list-item:has(ndd-text-cell:has-text("heeft_recht_op_zorgtoeslag"))').last();
    await actionItem.locator('ndd-button:has-text("Bewerk")').click();
    await page.waitForTimeout(300);

    const panel = page.locator('ndd-sheet');
    await expect(panel).toBeVisible();

    // Verify the operation type
    const typeSelect = panel.locator('[data-testid="operation-type-dropdown"] select');
    const currentType = await typeSelect.evaluate(el => el.value);
    expect(currentType).toBe('GREATER_THAN_OR_EQUAL');
  });
});
