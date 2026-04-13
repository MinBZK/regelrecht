import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane } from './helpers.js';
import yaml from 'js-yaml';
import { readFileSync } from 'fs';
import { resolve } from 'path';

/**
 * Create an article with machine_readable containing an action with an operation.
 * This lets us test the OperationSettings binding directly.
 */
function createFixtureWithAction() {
  const base = readFileSync(resolve(import.meta.dirname, 'fixtures/zorgtoeslag-stripped.yaml'), 'utf-8');
  const law = yaml.load(base);

  // Give article 2 an action with an ADD operation
  law.articles[2].machine_readable = {
    definitions: {
      standaard_waarde: { value: 100 },
    },
    execution: {
      parameters: [{ name: 'bsn', type: 'string', required: true }],
      input: [],
      output: [
        { name: 'resultaat', type: 'number' },
      ],
      actions: [
        {
          output: 'resultaat',
          value: {
            operation: 'ADD',
            values: [10, 20],
          },
        },
      ],
    },
  };

  return yaml.dump(law, { lineWidth: 80, noRefs: true });
}

test.describe('Operation binding', () => {
  test('changing operation type updates YAML', async ({ page }) => {
    const fixtureYaml = createFixtureWithAction();

    await page.route('**/api/corpus/laws/zorgtoeslagwet', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fixtureYaml })
    );
    await page.goto('/editor/zorgtoeslagwet');
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 10_000 });

    await selectArticle(page, '2');
    await page.waitForTimeout(300);

    // Click "Bewerk" on the action in the Acties section (last list item with "resultaat")
    const actionItems = page.locator('ndd-list-item:has(ndd-text-cell:has-text("resultaat"))');
    await actionItems.last().locator('ndd-button:has-text("Bewerk")').click();
    await page.waitForTimeout(300);

    // ActionSheet should be open
    const panel = page.locator('ndd-sheet');
    await expect(panel).toBeVisible();

    // Verify the operation type is currently ADD
    const typeSelect = panel.locator('[data-testid="operation-type-dropdown"] select');
    const currentType = await typeSelect.evaluate(el => el.value);
    expect(currentType).toBe('ADD');

    // Change type to MULTIPLY
    await typeSelect.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('change', { bubbles: true }));
    }, 'MULTIPLY');
    await page.waitForTimeout(100);

    // Save
    await panel.locator('ndd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    // Verify YAML
    const parsedYaml = await readYamlPane(page);
    expect(parsedYaml.execution.actions[0].value.operation).toBe('MULTIPLY');
  });

  test('changing literal value updates YAML', async ({ page }) => {
    const fixtureYaml = createFixtureWithAction();

    await page.route('**/api/corpus/laws/zorgtoeslagwet', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fixtureYaml })
    );
    await page.goto('/editor/zorgtoeslagwet');
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 10_000 });

    await selectArticle(page, '2');
    await page.waitForTimeout(300);

    // Click "Bewerk" on the action in the Acties section (last list item with "resultaat")
    const actionItems = page.locator('ndd-list-item:has(ndd-text-cell:has-text("resultaat"))');
    await actionItems.last().locator('ndd-button:has-text("Bewerk")').click();
    await page.waitForTimeout(300);

    const panel = page.locator('ndd-sheet');

    // Find value 1 input (should be 10)
    const value1Input = panel.locator('[data-testid="op-value-0"] ndd-text-field input');
    await value1Input.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, '42');
    await page.waitForTimeout(100);

    // Save
    await panel.locator('ndd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    // Verify YAML
    const parsedYaml = await readYamlPane(page);
    expect(parsedYaml.execution.actions[0].value.values[0]).toBe(42);
    expect(parsedYaml.execution.actions[0].value.values[1]).toBe(20);
  });

  test('adding a value via button updates YAML', async ({ page }) => {
    const fixtureYaml = createFixtureWithAction();

    await page.route('**/api/corpus/laws/zorgtoeslagwet', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fixtureYaml })
    );
    await page.goto('/editor/zorgtoeslagwet');
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 10_000 });

    await selectArticle(page, '2');
    await page.waitForTimeout(300);

    // Click "Bewerk" on the action in the Acties section (last list item with "resultaat")
    const actionItems = page.locator('ndd-list-item:has(ndd-text-cell:has-text("resultaat"))');
    await actionItems.last().locator('ndd-button:has-text("Bewerk")').click();
    await page.waitForTimeout(300);

    const panel = page.locator('ndd-sheet');

    // Click "Voeg waarde toe"
    await panel.locator('[data-testid="add-value-btn"]').click();
    await page.waitForTimeout(200);

    // Save
    await panel.locator('ndd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    // Verify YAML - should now have 3 values
    const parsedYaml = await readYamlPane(page);
    expect(parsedYaml.execution.actions[0].value.values).toHaveLength(3);
    expect(parsedYaml.execution.actions[0].value.values[2]).toBe(0); // Default new value
  });

  test('removing a value via minus button updates YAML', async ({ page }) => {
    const fixtureYaml = createFixtureWithAction();

    await page.route('**/api/corpus/laws/zorgtoeslagwet', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fixtureYaml })
    );
    await page.goto('/editor/zorgtoeslagwet');
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 10_000 });

    await selectArticle(page, '2');
    await page.waitForTimeout(300);

    // Click "Bewerk" on the action in the Acties section (last list item with "resultaat")
    const actionItems = page.locator('ndd-list-item:has(ndd-text-cell:has-text("resultaat"))');
    await actionItems.last().locator('ndd-button:has-text("Bewerk")').click();
    await page.waitForTimeout(300);

    const panel = page.locator('ndd-sheet');

    // Click minus button on first value (ndd-icon-button may be "not visible" to Playwright)
    const removeBtn = panel.locator('[data-testid="op-value-0"] ndd-icon-button[icon="minus"]');
    await removeBtn.evaluate(el => el.click());
    await page.waitForTimeout(200);

    // Save
    await panel.locator('ndd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    // Verify YAML - should now have 1 value (20 remains)
    const parsedYaml = await readYamlPane(page);
    expect(parsedYaml.execution.actions[0].value.values).toHaveLength(1);
    expect(parsedYaml.execution.actions[0].value.values[0]).toBe(20);
  });
});
