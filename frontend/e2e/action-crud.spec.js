import { test, expect } from '@playwright/test';
import {
  interceptLaw,
  gotoEditor,
  selectArticle,
  readYamlPane,
  setYamlPane,
  openSheet,
  openActionEditor,
  saveActionSheet,
} from './helpers.js';

test.describe('Action CRUD', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'wet_op_de_zorgtoeslag', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);
  });

  test('add an action with an arithmetic operation via the sheet', async ({ page }) => {
    await selectArticle(page, '5');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Click "Actie toevoegen" - a new action opens the sheet seeded with an
    // editable EQUALS stub (see the "blocks empty save" test below).
    await page.locator('[data-testid="add-action-btn"]').click();
    await page.waitForTimeout(300);

    const panel = openSheet(page);
    await expect(panel).toBeVisible();

    // Set output name
    const outputField = panel.locator('[data-testid="action-output-field"] input');
    await outputField.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'bevoegd_gezag');
    await page.waitForTimeout(100);

    // Switch the operation to an arithmetic ADD (needs no subject variable,
    // so it can be completed on an otherwise-empty article) and give it a
    // value so the save guard accepts it.
    const typeSelect = panel.locator('[data-testid="operation-type-dropdown"] select');
    await typeSelect.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('change', { bubbles: true }));
    }, 'ADD');
    await page.waitForTimeout(150);

    await panel.locator('[data-testid="add-value-btn"]').click();
    await page.waitForTimeout(150);

    const value0 = panel.locator('[data-testid="op-value-0"] nldd-text-field input');
    await value0.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, '5');
    await page.waitForTimeout(100);

    // Save - a complete operation now passes the guard and the sheet closes.
    await saveActionSheet(page);
    await expect(openSheet(page)).toHaveCount(0);

    // Verify YAML
    const yaml = await readYamlPane(page);
    expect(yaml.execution.actions).toHaveLength(1);
    expect(yaml.execution.actions[0].output).toBe('bevoegd_gezag');
    expect(yaml.execution.actions[0].value.operation).toBe('ADD');
    expect(yaml.execution.actions[0].value.values).toContain(5);
  });

  test('author an action with a literal value via the YAML pane', async ({ page }) => {
    await selectArticle(page, '8');

    // Init machine_readable, then author a literal-value action through the
    // YAML pane - the editor's manual escape hatch for shapes the structured
    // form does not build (a bare literal output value).
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    await setYamlPane(page, `definitions: {}
execution:
  parameters: []
  input: []
  output: []
  actions:
    - output: wet_naam
      value: Wet op de zorgtoeslag
`);
    await page.waitForTimeout(300);

    // Verify YAML round-trips.
    const yaml = await readYamlPane(page);
    expect(yaml.execution.actions).toHaveLength(1);
    expect(yaml.execution.actions[0].output).toBe('wet_naam');
    expect(yaml.execution.actions[0].value).toBe('Wet op de zorgtoeslag');

    // The action renders in the machine pane and its sheet shows the literal
    // value verbatim (the ActionSheet's direct-value display).
    await openActionEditor(page, 'wet_naam');
    await expect(openSheet(page)).toContainText('Wet op de zorgtoeslag');
  });

  test('adding an action blocks saving until its seeded operation is filled', async ({ page }) => {
    await selectArticle(page, '2');

    // Init machine_readable
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Add an action and set only its output - the seeded EQUALS operation is
    // still empty, so saving must be rejected (an incomplete operation would
    // produce YAML the engine cannot execute).
    await page.locator('[data-testid="add-action-btn"]').click();
    await page.waitForTimeout(300);

    const panel = openSheet(page);
    const outputField = panel.locator('[data-testid="action-output-field"] input');
    await outputField.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'hoogte_zorgtoeslag');
    await page.waitForTimeout(100);

    await saveActionSheet(page);
    await page.waitForTimeout(200);

    // The sheet stays open and the middle pane surfaces the guard message.
    await expect(openSheet(page)).toBeVisible();
    const banner = page.locator('nldd-banner[variant="critical"]');
    await expect(banner).toHaveAttribute('text', /Operatie 'EQUALS' is nog niet ingevuld/);
  });
});
