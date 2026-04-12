/**
 * Edit → re-execute loop end-to-end.
 *
 * Verifies the demo flow the editor was built for:
 *   1. Open zorgtoeslagwet in the editor.
 *   2. Observe the *Minderjarige heeft geen recht op zorgtoeslag* scenario is
 *      red (badge = ✗) — age check isn't in the law's machine_readable.
 *   3. Edit article 2's machine_readable via the middle-pane YAML editor to
 *      add a leeftijd input + an AGE-based condition to the existing AND.
 *   4. ScenarioBuilder auto-reexecutes against the edited YAML.
 *   5. Observe the scenario badge is now green (badge = ✓).
 *
 * This is the smoke-test for the propagation chain we just wired up:
 *   machineReadable edit → currentLawYaml computed → engine reload →
 *   ScenarioBuilder lawYaml prop → dependency reload → auto-execute.
 *
 * All corpus laws, the scenarios list, the scenario feature file, and the
 * PUT save endpoint are mocked from the on-disk corpus directory so the
 * spec doesn't need a running editor-api. This keeps it CI-friendly and
 * reproduces the real dependency graph.
 */
import { test, expect } from '@playwright/test';
import yaml from 'js-yaml';
import { loadCorpus, loadScenario, mockCorpusApi } from './helpers-corpus.js';

test.describe('Edit → re-execute loop', () => {
  test.beforeEach(async ({ page }) => {
    // Clear localStorage tabs to avoid bleed-over between test runs.
    await page.addInitScript(() => {
      try {
        window.localStorage.removeItem('regelrecht-open-tabs');
      } catch { /* ignore */ }
    });
  });

  test('Minderjarige scenario goes red → green after adding age check', async ({ page }) => {
    const corpus = loadCorpus();
    const zorgtoeslag = corpus.get('zorgtoeslagwet');
    expect(zorgtoeslag, 'zorgtoeslagwet must exist in the test corpus').toBeTruthy();

    const scenarioFilename = 'eligibility.feature';
    const scenarioText = loadScenario(zorgtoeslag.path, scenarioFilename);
    expect(scenarioText, 'eligibility.feature must exist').toBeTruthy();

    await mockCorpusApi(
      page,
      corpus,
      { id: 'zorgtoeslagwet', scenarioFilename },
      scenarioText,
    );

    // Navigate directly to article 2 via the query param — that's where
    // heeft_recht_op_zorgtoeslag lives and where we need to edit.
    await page.goto('/editor/zorgtoeslagwet?article=2');

    // Wait for the document tab bar to render — articles loaded.
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 15_000 });

    const minorHeader = page
      .locator('.sb-accordion-header')
      .filter({ hasText: 'Minderjarige' });
    await expect(minorHeader).toBeVisible({ timeout: 30_000 });

    // Wait until the badge appears (either ✓ or ✗) — meaning execution
    // completed. The badge span has class sb-badge--pass or sb-badge--fail.
    await minorHeader
      .locator('.sb-badge--pass, .sb-badge--fail')
      .first()
      .waitFor({ timeout: 30_000 });

    // Initial state: scenario is failed (age check not in the law).
    await expect(minorHeader).toHaveClass(/sb-header--fail/);

    // Toggle the middle pane to YAML view. ndd-segmented-control-item is
    // a custom element whose click target lives in shadow DOM, so instead
    // of clicking we synthesize the change event the way EditorApp's
    // `onMiddlePaneChange` handler expects: it reads `event.target.value`
    // first, then falls back to `event.detail[0]`.
    await page.locator('[data-testid="middle-pane-toggle"]').evaluate((el) => {
      el.value = 'yaml';
      el.dispatchEvent(new Event('change', { bubbles: true }));
    });
    // Wait for Vue to re-render the YAML pane.
    await page.waitForSelector('.editor-yaml-textarea', { timeout: 5000 });

    // Grab the current YAML (article 2's machine_readable), parse it,
    // surgically add the leeftijd input and the AND condition, and write
    // it back into the textarea.
    const textarea = page.locator('.editor-yaml-textarea');
    await expect(textarea).toBeVisible();

    const originalYaml = await textarea.inputValue();
    expect(originalYaml).toContain('heeft_recht_op_zorgtoeslag');
    const mr = yaml.load(originalYaml);

    // Inject leeftijd input (sourced from BRP with a literal peildatum).
    // BRP art 1.2 requires both bsn and peildatum; we use the scenario's
    // calculation date as a literal here.
    //
    // NOTE: a literal date is intentional for test isolation — the spec must
    // remain stable regardless of the real calculation date at CI time. The
    // canonical production pattern (see `kieswet`) references a parameter
    // like `peildatum: $verkiezingsdatum` so the date tracks the runtime
    // context; don't copy the literal form into corpus laws.
    mr.execution.input.push({
      name: 'leeftijd',
      type: 'number',
      source: {
        regulation: 'wet_basisregistratie_personen',
        output: 'leeftijd',
        parameters: {
          bsn: '$bsn',
          peildatum: '2025-01-01',
        },
      },
    });

    // Append the age condition to the heeft_recht_op_zorgtoeslag AND.
    // Assert the action is shaped the way we expect before mutating so a
    // future refactor (e.g. top-level op change) produces a legible error
    // instead of a bare `Cannot read properties of undefined` on .push.
    const heeftRecht = mr.execution.actions.find(
      (a) => a.output === 'heeft_recht_op_zorgtoeslag',
    );
    expect(heeftRecht, 'heeft_recht_op_zorgtoeslag action must exist').toBeTruthy();
    expect(
      heeftRecht.value?.operation,
      'heeft_recht action must be an AND at the top level',
    ).toBe('AND');
    expect(Array.isArray(heeftRecht.value?.conditions)).toBe(true);
    heeftRecht.value.conditions.push({
      operation: 'GREATER_THAN_OR_EQUAL',
      subject: '$leeftijd',
      value: 18,
    });

    const editedYaml = yaml.dump(mr, { lineWidth: 80, noRefs: true });

    // Fill the textarea by dispatching an input event; the editor's
    // onYamlInput handler parses the text and updates machineReadable.
    await textarea.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, editedYaml);

    // Toggle back to the form view so the scenarios accordion mounts
    // again. The middle pane only shows one of the two views at a time;
    // remounting ScenarioBuilder kicks off its immediate `lawYaml` watch,
    // which reloads dependencies against the edited law and re-executes.
    await page.locator('[data-testid="middle-pane-toggle"]').evaluate((el) => {
      el.value = 'form';
      el.dispatchEvent(new Event('change', { bubbles: true }));
    });

    // Re-execution fires via the currentLawYaml → ScenarioBuilder lawYaml
    // prop chain. Allow the engine + dependency reload + scenario run to
    // complete.
    const minorHeaderAfter = page
      .locator('.sb-accordion-header')
      .filter({ hasText: 'Minderjarige' })
      .first();
    await expect(minorHeaderAfter).toHaveClass(/sb-header--pass/, { timeout: 60_000 });
  });
});
