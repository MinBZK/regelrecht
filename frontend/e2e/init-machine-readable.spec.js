import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane } from './helpers.js';

test.describe('Init machine_readable', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'zorgtoeslagwet', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);
  });

  test('shows init button when article has no machine_readable', async ({ page }) => {
    // Article 1a has no machine_readable in stripped fixture
    await selectArticle(page, '1a');

    // Should show the "no data" message with init button
    const noMr = page.locator('[data-testid="no-machine-readable"]');
    await expect(noMr).toBeVisible();
    await expect(noMr).toContainText('Geen machine-leesbare gegevens');

    const initBtn = page.locator('[data-testid="init-mr-btn"]');
    await expect(initBtn).toBeVisible();
  });

  test('clicking init creates empty machine_readable scaffold', async ({ page }) => {
    await selectArticle(page, '1a');

    // Click init button
    await page.locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // The machine-readable section should now be visible
    const mrPane = page.locator('[data-testid="machine-readable"]');
    await expect(mrPane).toBeVisible();

    // Section headers should appear (editable mode shows empty sections)
    await expect(page.locator('[data-testid="section-definitions"]')).toBeVisible();
    await expect(page.locator('[data-testid="section-parameters"]')).toBeVisible();
    await expect(page.locator('[data-testid="section-inputs"]')).toBeVisible();
    await expect(page.locator('[data-testid="section-outputs"]')).toBeVisible();
    await expect(page.locator('[data-testid="section-actions"]')).toBeVisible();

    // YAML pane should show the scaffold
    const yaml = await readYamlPane(page);
    expect(yaml).toBeTruthy();
    expect(yaml).toHaveProperty('definitions');
    expect(yaml).toHaveProperty('execution');
    expect(yaml.execution).toHaveProperty('parameters');
    expect(yaml.execution).toHaveProperty('input');
    expect(yaml.execution).toHaveProperty('output');
    expect(yaml.execution).toHaveProperty('actions');
  });
});
