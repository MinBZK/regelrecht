// De bewijsronde door het aanvraagportaal: een aanvrager kiezen, als haar een
// aanvraag indienen, en na een besluit op "inzicht in je aanvraag" haar
// beschikking zien. Elke naam die hier staat komt uit het portaal-blok van de
// publieke wereld.
//
// Eigen sessie en dus eigen wereld: de bewijsronde door de wereld zelf
// (`publieke-wereld.spec.js`) raakt dit verhaal niet, en omgekeerd.

import { test, expect } from '@playwright/test';

import { grams, openSession } from './world.js';

const AANVRAGER = 'Aanvrager A (fictief)';
const BSN = '999993653';
const TWEEDE = 'Aanvrager B (fictief)';
const TWEEDE_BSN = '999990019';
const DERDE = 'Aanvrager C (fictief)';
const DERDE_BSN = '999990032';

test.describe.configure({ mode: 'serial' });

test.describe('aanvraagportaal', () => {
  /** @type {Awaited<ReturnType<typeof openSession>>} */
  let s;
  let page;
  /** Wat de kaart met de beschikking van de eerste aanvrager toonde. */
  let beschikkingA = '';

  test.beforeAll(async ({ browser }) => {
    s = await openSession(browser, '/#/portaal');
    page = s.page;
  });

  test.afterEach(() => {
    expect(s.pageErrors).toEqual([]);
  });

  test.afterAll(async () => {
    await s?.close();
  });

  /** De keuzelijst "Aanvragen als", op de optie die alleen zij heeft. */
  const persona = () => page.locator('select', { has: page.locator('option', { hasText: 'Kies een aanvrager' }) });

  /** Alle tekst van één kaart, ook wat in attributen en shadow roots staat. */
  const cardText = async (heading) =>
    await page
      .locator('nldd-card', { has: page.getByRole('heading', { name: heading }) })
      .evaluate((card) => {
        const out = [card.innerText];
        const walk = (root) => {
          for (const el of root.querySelectorAll('*')) {
            for (const attr of ['text', 'supporting-text']) {
              const value = el.getAttribute && el.getAttribute(attr);
              if (value) out.push(value);
            }
            if (el.shadowRoot) {
              out.push(el.shadowRoot.textContent || '');
              walk(el.shadowRoot);
            }
          }
        };
        walk(card);
        return out.join('\n');
      });

  test('P1: zonder aanvrager vraagt het portaal om een keuze', async () => {
    await expect(page.getByRole('heading', { name: 'Aanvraagportaal' })).toBeVisible();
    await expect(persona()).toBeVisible();
    await expect(page.locator('nldd-card')).toHaveCount(0);
    expect(await s.textAll()).toMatch(/Kies als wie u aanvraagt/);
  });

  test('P2: een aanvrager kiezen vult het formulier met haar gegevens, en legt niets vast', async () => {
    await persona().selectOption({ label: AANVRAGER });
    await expect(page.locator('nldd-card input[aria-label="Bsn"]')).toHaveValue(BSN);
    const world = await s.world();
    expect(world.persona).toBe('aanvrager-a');
    expect(world.journal).toHaveLength(0);
    expect(world.crossings).toHaveLength(0);
    // Alleen de acties van de aanvrager: de besluiten van de uitvoerder staan
    // hier niet.
    const actors = new Set(world.actions.filter((a) => a.actor === 'burger').map((a) => a.label));
    await expect(page.locator('nldd-card')).toHaveCount(actors.size);
  });

  test('P3: de aanvraag indienen geeft een bevestiging met een link naar inzicht', async () => {
    await page.locator('nldd-card form button[type=submit]').first().click();
    await s.worldWhen((w) => grams(w, 'burger', 'aanvragen').length === 1, 'de aanvraag landt bij de burger');
    await expect.poll(async () => /ingediend/.test(await s.textAll())).toBe(true);
    await page.getByRole('link', { name: 'Bekijk inzicht in je aanvraag' }).click();
    await expect(page).toHaveURL(/#\/inzicht$/);
  });

  test('P4: vóór een besluit is er over de beschikking nog niets bekend', async () => {
    await expect(page.getByRole('heading', { name: 'Beschikking zorgtoeslag' })).toBeVisible();
    await expect.poll(async () => await cardText('Beschikking zorgtoeslag')).toMatch(/Nog niets bekend/);
  });

  test('P5: na het besluit toont inzicht de beschikking', async () => {
    // Het besluit is niet aan de aanvrager: dat gebeurt achter de schermen.
    await page.goto('/#/wereld');
    await page.waitForTimeout(800);
    await s.advance('01-03-2024');
    await s.actionForm('toeslagen.toekenning').locator('button[type=submit]').click();
    await s.worldWhen((w) => grams(w, 'toeslagen', 'beschikkingen').length === 1, 'het besluit is genomen');

    await page.getByRole('link', { name: 'Inzicht in je aanvraag', exact: true }).click();
    await expect(page).toHaveURL(/#\/inzicht$/);
    await expect
      .poll(async () => await cardText('Beschikking zorgtoeslag'), { timeout: 20_000 })
      .toMatch(/Hoogte zorgtoeslag/);
    beschikkingA = await cardText('Beschikking zorgtoeslag');
    expect(beschikkingA).not.toMatch(/Nog niets bekend/);
    // En de keuze is blijven staan, over de pagina's heen.
    await expect(persona()).toHaveValue('aanvrager-a');
  });

  test('P6: een tweede aanvrager heeft haar eigen inzicht, en na een besluit haar eigen beschikking', async () => {
    // Wisselen op inzicht: over haar is nog niets besloten.
    await persona().selectOption({ label: TWEEDE });
    await expect.poll(async () => await cardText('Beschikking zorgtoeslag')).toMatch(/Nog niets bekend/);

    // De wissel geldt ook op het portaal: haar gegevens in het formulier.
    await page.getByRole('link', { name: 'Aanvraagportaal', exact: true }).click();
    await expect(page).toHaveURL(/#\/portaal$/);
    await expect(persona()).toHaveValue('aanvrager-b');
    await expect(page.locator('nldd-card input[aria-label="Bsn"]')).toHaveValue(TWEEDE_BSN);
    await page.locator('nldd-card form button[type=submit]').first().click();
    await s.worldWhen((w) => grams(w, 'burger', 'aanvragen').length === 2, 'haar aanvraag landt bij de burger');

    // Het besluit, weer achter de schermen: de uitvoerder beslist op de
    // aanvraag die het laatst binnenkwam, en dat is de hare.
    await page.goto('/#/wereld');
    await page.waitForTimeout(800);
    await expect(s.actionForm('toeslagen.toekenning').locator('input[aria-label="Bsn"]')).toHaveValue(TWEEDE_BSN);
    await s.actionForm('toeslagen.toekenning').locator('button[type=submit]').click();
    await s.worldWhen((w) => grams(w, 'toeslagen', 'beschikkingen').length === 2, 'het besluit over haar is genomen');

    await page.getByRole('link', { name: 'Inzicht in je aanvraag', exact: true }).click();
    await expect(page).toHaveURL(/#\/inzicht$/);
    await expect(persona()).toHaveValue('aanvrager-b');
    await expect
      .poll(async () => await cardText('Beschikking zorgtoeslag'), { timeout: 20_000 })
      .toMatch(/Hoogte zorgtoeslag/);
    // Een ander inkomen, een ander bedrag: dit is haar beschikking en niet die
    // van de eerste aanvrager.
    expect(await cardText('Beschikking zorgtoeslag')).not.toBe(beschikkingA);
  });

  test('P7: een aanvrager zonder recht ziet haar besluit als afwijzing, met de grond bovenaan', async () => {
    await persona().selectOption({ label: DERDE });
    await expect.poll(async () => await cardText('Beschikking zorgtoeslag')).toMatch(/Nog niets bekend/);

    await page.getByRole('link', { name: 'Aanvraagportaal', exact: true }).click();
    await expect(page).toHaveURL(/#\/portaal$/);
    await expect(page.locator('nldd-card input[aria-label="Bsn"]')).toHaveValue(DERDE_BSN);
    await page.locator('nldd-card form button[type=submit]').first().click();
    await s.worldWhen((w) => grams(w, 'burger', 'aanvragen').length === 3, 'haar aanvraag landt bij de burger');

    await page.goto('/#/wereld');
    await page.waitForTimeout(800);
    await expect(s.actionForm('toeslagen.toekenning').locator('input[aria-label="Bsn"]')).toHaveValue(DERDE_BSN);
    await s.actionForm('toeslagen.toekenning').locator('button[type=submit]').click();
    // Het beeld zelf: het besluit over haar is een afwijzing, uit de wet.
    const world = await s.worldWhen(
      (w) => grams(w, 'toeslagen', 'beschikkingen').length === 3,
      'het besluit over haar is genomen',
    );
    const afwijzingen = grams(world, 'toeslagen', 'beschikkingen').filter((gram) =>
      JSON.stringify(gram.fields?.decision_type ?? null).includes('AFWIJZING'),
    );
    expect(afwijzingen).toHaveLength(1);

    await page.getByRole('link', { name: 'Inzicht in je aanvraag', exact: true }).click();
    await expect(page).toHaveURL(/#\/inzicht$/);
    await expect(persona()).toHaveValue('aanvrager-c');
    const card = page.locator('nldd-card', { has: page.getByRole('heading', { name: 'Beschikking zorgtoeslag' }) });
    await expect(card.locator('nldd-banner[text="Afgewezen"]')).toBeVisible({ timeout: 20_000 });
    await expect(card.locator('nldd-banner nldd-text-cell[text="Heeft recht op zorgtoeslag"]')).toHaveAttribute(
      'supporting-text',
      /waarde: nee · artikel 2/,
    );
    await expect(card.getByRole('heading', { name: 'Berekend, niet toegekend' })).toBeVisible();
    // Het bedrag staat onder "berekend", het besluittype als gewone regel
    // eronder: twee lijsten, in die volgorde.
    const lijsten = card.locator('nldd-list[variant="box-tinted"]');
    await expect(lijsten).toHaveCount(2);
    await expect(lijsten.nth(0).locator('nldd-text-cell[text="Hoogte zorgtoeslag"]')).toHaveCount(1);
    await expect(lijsten.nth(1).locator('nldd-text-cell[text="Decision type"]')).toHaveCount(1);
  });
});
