import { test, expect } from '@playwright/test';
import {
  gotoEditor,
  selectArticle,
  readYamlPane,
  readYamlSource,
  setYamlPane,
  openSheet,
  openActionEditor,
  saveActionSheet,
} from './helpers.js';
import * as yaml from 'js-yaml';
import { readFileSync } from 'fs';
import { resolve } from 'path';

/**
 * Create a fixture with article 2 having definitions, params, inputs, outputs
 * but no actions yet - so we can test building complex actions from scratch.
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
  test('AND operation with comparison conditions round-trips and renders', async ({ page }) => {
    const fixtureYaml = createFixtureWithMetadata();

    await page.route('**/corpus/laws/wet_op_de_zorgtoeslag', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fixtureYaml })
    );
    await gotoEditor(page);

    await selectArticle(page, '2');
    await page.waitForTimeout(300);

    // Author the AND operation through the YAML pane. The structured form
    // seeds new actions with an empty EQUALS stub and won't build a nested
    // boolean tree from scratch, so YAML is the authoring path here.
    const currentYaml = await readYamlSource(page);
    const updatedYaml = currentYaml.replace(
      'actions: []',
      `actions:
    - output: heeft_recht_op_zorgtoeslag
      value:
        operation: AND
        conditions:
          - operation: GREATER_THAN_OR_EQUAL
            subject: $leeftijd
            value: 18
          - operation: EQUALS
            subject: $is_verzekerde
            value: true`
    );
    await setYamlPane(page, updatedYaml);
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

    // Open the ActionSheet and verify the operation tree renders. It opens on
    // the root operation, so the type dropdown shows AND.
    await openActionEditor(page, 'heeft_recht_op_zorgtoeslag');
    const panel = openSheet(page);
    await expect(panel).toBeVisible();

    const typeSelect = panel.locator('[data-testid="operation-type-dropdown"] select');
    expect(await typeSelect.evaluate(el => el.value)).toBe('AND');
  });

  test('turning a value into an empty nested operation blocks the save', async ({ page }) => {
    const fixtureYaml = createFixtureWithMetadata();

    await page.route('**/corpus/laws/wet_op_de_zorgtoeslag', route =>
      route.fulfill({ status: 200, contentType: 'text/yaml', body: fixtureYaml })
    );
    await gotoEditor(page);

    await selectArticle(page, '2');
    await page.waitForTimeout(300);

    // Author an action with a MAX operation over a single literal value.
    const currentYaml = await readYamlSource(page);
    const updatedYaml = currentYaml.replace(
      'actions: []',
      `actions:
    - output: hoogte_zorgtoeslag
      value:
        operation: MAX
        values:
          - 0`
    );
    await setYamlPane(page, updatedYaml);
    await page.waitForTimeout(300);

    await openActionEditor(page, 'hoogte_zorgtoeslag');
    const panel = openSheet(page);

    // Convert the literal value into a nested operation via its row-actions
    // menu. changeValueKind seeds an empty `ADD { values: [] }` - structurally
    // incomplete on purpose.
    await panel
      .locator('[data-testid="op-value-0"] nldd-menu-item[text="Operatie"]')
      .first()
      .evaluate((el) => el.click());
    await page.waitForTimeout(200);

    // Saving must be rejected: an empty nested operation would produce YAML
    // the engine cannot execute.
    await saveActionSheet(page);
    await page.waitForTimeout(200);

    await expect(panel).toBeVisible();
    const banner = page.locator('nldd-banner[variant="critical"]');
    await expect(banner).toHaveAttribute('text', /Operatie 'ADD' is nog niet ingevuld/);
  });
});
