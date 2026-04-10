import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane } from './helpers.js';
import yaml from 'js-yaml';
import { readFileSync } from 'fs';
import { resolve } from 'path';

/**
 * Create a fixture with article 2 having definitions, params, inputs, outputs
 * but no actions yet — so we can test building complex actions from scratch.
 */
function createFixtureWithMetadata() {
  const base = readFileSync(resolve(import.meta.dirname, 'fixtures/zorgtoeslag-stripped.yaml'), 'utf-8');
  const law = yaml.load(base);

  law.articles[2].machine_readable = {
    definitions: {
      drempelinkomen_alleenstaande: { value: 3971900 },
      percentage_drempelinkomen_alleenstaande: { value: 0.01896 },
    },
    execution: {
      parameters: [{ name: 'bsn', type: 'string', required: true }],
      input: [
        { name: 'leeftijd', type: 'number', source: { regulation: 'wet_basisregistratie_personen', output: 'leeftijd', parameters: { bsn: '$bsn' } } },
        { name: 'is_verzekerde', type: 'boolean', source: { regulation: 'zorgverzekeringswet', output: 'is_verzekerd', parameters: { bsn: '$bsn' } } },
      ],
      output: [
        { name: 'heeft_recht_op_zorgtoeslag', type: 'boolean' },
        { name: 'hoogte_zorgtoeslag', type: 'amount', type_spec: { unit: 'eurocent' } },
      ],
      actions: [],
    },
  };

  return yaml.dump(law, { lineWidth: 80, noRefs: true });
}

test.describe('Complex actions', () => {
  test('add action with AND operation containing comparison conditions', async ({ page }) => {
    const fixtureYaml = createFixtureWithMetadata();

    await page.route('**/api/corpus/laws/zorgtoeslagwet', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fixtureYaml })
    );
    await page.goto('/editor.html?law=zorgtoeslagwet');
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 10_000 });

    await selectArticle(page, '2');
    await page.waitForTimeout(300);

    // Add an action
    await page.locator('[data-testid="add-action-btn"]').click();
    await page.waitForTimeout(300);

    const panel = page.locator('ndd-sheet');

    // Set output
    const outputField = panel.locator('[data-testid="action-output-field"] input');
    await outputField.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'heeft_recht_op_zorgtoeslag');
    await page.waitForTimeout(100);

    // New actions have value='' — no OperationSettings shown
    // We need to close and use YAML editing to set up the initial operation structure
    await panel.locator('ndd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    // Edit YAML directly to set up the AND operation with conditions
    const textarea = page.locator('.editor-yaml-textarea');
    const currentYaml = await textarea.inputValue();

    // Replace the action's value with an AND operation
    const updatedYaml = currentYaml.replace(
      "value: ''",
      `value:
            operation: AND
            conditions:
              - operation: GREATER_THAN_OR_EQUAL
                subject: $leeftijd
                value: 18
              - operation: EQUALS
                subject: $is_verzekerde
                value: true`
    );

    await textarea.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, updatedYaml);
    await page.waitForTimeout(300);

    // Verify YAML round-trips correctly
    const parsedYaml = await readYamlPane(page);
    const action = parsedYaml.execution.actions[0];
    expect(action.output).toBe('heeft_recht_op_zorgtoeslag');
    expect(action.value.operation).toBe('AND');
    expect(action.value.conditions).toHaveLength(2);
    expect(action.value.conditions[0].operation).toBe('GREATER_THAN_OR_EQUAL');
    expect(action.value.conditions[0].subject).toBe('$leeftijd');
    expect(action.value.conditions[0].value).toBe(18);
    expect(action.value.conditions[1].operation).toBe('EQUALS');
    expect(action.value.conditions[1].subject).toBe('$is_verzekerde');
    expect(action.value.conditions[1].value).toBe(true);

    // Now open the ActionSheet and verify the operation tree renders
    const actionItem = page.locator('ndd-list-item:has(ndd-text-cell:has-text("heeft_recht_op_zorgtoeslag"))').last();
    await actionItem.locator('ndd-button:has-text("Bewerk")').click();
    await page.waitForTimeout(300);

    // ActionSheet should show the AND operation
    const panel2 = page.locator('ndd-sheet');
    await expect(panel2).toBeVisible();

    // The ActionSheet initially selects the deepest operation in the tree.
    // Verify the operation settings panel rendered (has a type dropdown)
    const typeSelect = panel2.locator('[data-testid="operation-type-dropdown"] select');
    const currentType = await typeSelect.evaluate(el => el.value);
    // Should be one of the comparison ops (the leaves of the AND tree)
    expect(['EQUALS', 'GREATER_THAN_OR_EQUAL']).toContain(currentType);

    // Verify the "Bovenliggende operaties" section is shown (since we're viewing a child)
    await expect(panel2.locator('text=Bovenliggende operaties')).toBeVisible();

    // Close
    await panel2.locator('ndd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(200);
  });

  test('add nested operation via button and verify in YAML', async ({ page }) => {
    const fixtureYaml = createFixtureWithMetadata();

    await page.route('**/api/corpus/laws/zorgtoeslagwet', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fixtureYaml })
    );
    await page.goto('/editor.html?law=zorgtoeslagwet');
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 10_000 });

    await selectArticle(page, '2');
    await page.waitForTimeout(300);

    // Set up an action with a MAX operation via YAML editing
    const textarea = page.locator('.editor-yaml-textarea');
    const currentYaml = await textarea.inputValue();

    // Replace "actions: []" with a real action
    const updatedYaml = currentYaml.replace(
      'actions: []',
      `actions:
    - output: hoogte_zorgtoeslag
      value:
        operation: MAX
        values:
          - 0`
    );

    await textarea.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, updatedYaml);
    await page.waitForTimeout(300);

    // Open the action sheet
    const actionItem = page.locator('ndd-list-item:has(ndd-text-cell:has-text("hoogte_zorgtoeslag"))').last();
    await actionItem.locator('ndd-button:has-text("Bewerk")').click();
    await page.waitForTimeout(300);

    const panel = page.locator('ndd-sheet');

    // Click "Voeg operatie toe" to add a nested ADD (empty values[])
    await panel.locator('[data-testid="add-nested-op-btn"]').evaluate(el => el.click());
    await page.waitForTimeout(200);

    // Save should be rejected because the nested ADD has no values yet —
    // saving a structurally-incomplete operation would produce YAML the
    // engine cannot execute.
    await panel.locator('ndd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    // The action sheet should still be open (save was blocked)
    await expect(panel).toBeVisible();

    // The middle pane should display the parse error from handleActionSave
    const error = page.locator('.editor-parse-error-detail');
    await expect(error).toContainText("Operatie 'ADD' is nog niet ingevuld");
  });
});
