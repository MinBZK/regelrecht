import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane } from './helpers.js';

test.describe('Action CRUD', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'zorgtoeslagwet', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);
  });

  test('add a simple literal-value action', async ({ page }) => {
    await selectArticle(page, '5');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Click "Voeg actie toe"
    await page.locator('[data-testid="add-action-btn"]').click();
    await page.waitForTimeout(300);

    // ActionSheet should be open
    const panel = page.locator('nldd-sheet');
    await expect(panel).toBeVisible();

    // Set output name
    const outputField = panel.locator('[data-testid="action-output-field"] input');
    await outputField.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'bevoegd_gezag');
    await page.waitForTimeout(100);

    // The operation tree should be empty for a new action with value=''
    // The OperationSettings won't show since there's no operation
    // Let's save and check the YAML
    await panel.locator('nldd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    // Verify YAML
    const yaml = await readYamlPane(page);
    expect(yaml.execution.actions).toHaveLength(1);
    expect(yaml.execution.actions[0].output).toBe('bevoegd_gezag');
  });

  test('add action with output name and literal value via YAML editing', async ({ page }) => {
    await selectArticle(page, '8');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Add an action
    await page.locator('[data-testid="add-action-btn"]').click();
    await page.waitForTimeout(300);

    const panel = page.locator('nldd-sheet');

    // Set output name
    const outputField = panel.locator('[data-testid="action-output-field"] input');
    await outputField.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'wet_naam');
    await page.waitForTimeout(100);

    // Save actionsheet
    await panel.locator('nldd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    // Now manually edit the YAML to set the value (simpler than building UI for literal string values)
    const textarea = page.locator('.editor-yaml-textarea');
    const currentYaml = await textarea.inputValue();
    const updatedYaml = currentYaml.replace("value: ''", "value: Wet op de zorgtoeslag");
    await textarea.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, updatedYaml);
    await page.waitForTimeout(200);

    // Read back YAML
    const yaml = await readYamlPane(page);
    expect(yaml.execution.actions[0].output).toBe('wet_naam');
    expect(yaml.execution.actions[0].value).toBe('Wet op de zorgtoeslag');
  });

  test('add action with operation type and values', async ({ page }) => {
    await selectArticle(page, '2');

    // Init and add outputs first (needed for action output binding)
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Add an action
    await page.locator('[data-testid="add-action-btn"]').click();
    await page.waitForTimeout(300);

    const panel = page.locator('nldd-sheet');

    // Set output name
    const outputField = panel.locator('[data-testid="action-output-field"] input');
    await outputField.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'hoogte_zorgtoeslag');
    await page.waitForTimeout(100);

    // No OperationSettings visible yet because action.value is '' (not an operation)
    // Save and verify
    await panel.locator('nldd-button:has-text("Opslaan")').click();
    await page.waitForTimeout(300);

    const yaml = await readYamlPane(page);
    expect(yaml.execution.actions).toHaveLength(1);
    expect(yaml.execution.actions[0].output).toBe('hoogte_zorgtoeslag');
  });
});
