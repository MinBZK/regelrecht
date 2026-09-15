// De bewijsronde door de publieke wereld: één verhaal van aanvraag tot
// vaststelling, met onderweg een bewering per stap. De namen zijn de check-id's
// waaronder deze ronde met de hand liep (L = lay-out, F = formulier, S/D/V =
// vastlegging, J = journaal, X = celgrens, A = actiestand, M = lexostatus,
// G = grammen, B = betaling, I = instelling, N/E = wat er mis mag gaan), zodat
// een rode regel in CI dezelfde naam draagt als het bewijs.
//
// De volgorde is het verhaal: `serial`, één worker, één sessie en dus één
// wereld. Valt er één om, dan slaat de rest over — de wereld staat dan niet
// meer waar de volgende check hem verwacht.

import { test, expect } from '@playwright/test';

import { grams, hasDelta, kindIs, openSession } from './world.js';

// De burger uit het wereldbestand. Een testBSN uit de officiële reeks, geen
// persoon.
const BSN = '999993653';

test.describe.configure({ mode: 'serial' });

test.describe('publieke wereld', () => {
  /** @type {Awaited<ReturnType<typeof openSession>>} */
  let s;
  let page;
  let forms;
  let world;
  let fields;
  let earlyDecision;

  test.beforeAll(async ({ browser }) => {
    s = await openSession(browser);
    page = s.page;
    forms = page.locator('form');
  });

  test.afterAll(async () => {
    await s?.close();
  });

  test('L1: de bediening staat boven de cellen', async () => {
    const geo = await page.evaluate(() => {
      const tabs = document.querySelector('nldd-tab-bar');
      const leaves = [...document.querySelectorAll('*')].filter(
        (el) => el.children.length === 0 && /^\s*Cel\s*$/.test(el.textContent || ''),
      );
      return {
        tabsY: tabs ? tabs.getBoundingClientRect().top : null,
        cellY: leaves.length ? leaves[0].getBoundingClientRect().top : null,
      };
    });
    expect(geo.tabsY, 'geen tabbalk gevonden').not.toBeNull();
    expect(geo.cellY, 'geen celkop gevonden').not.toBeNull();
    expect(geo.tabsY).toBeLessThan(geo.cellY);
  });

  test('L2: de pagina scrolt niet horizontaal', async () => {
    const geo = await page.evaluate(() => ({
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
    }));
    expect(geo.scrollWidth).toBeLessThanOrEqual(geo.innerWidth + 1);
  });

  test('L3: het favicon wordt geleverd', async () => {
    const ico = await s.api('GET', '/favicon.ico');
    const svg = await s.api('GET', '/favicon.svg');
    expect(
      ico.status === 200 || svg.status === 200,
      `/favicon.ico=${ico.status} /favicon.svg=${svg.status}`,
    ).toBe(true);
  });

  test('F1: de aanvraag is voorgevuld uit het wereldbestand', async () => {
    world = await s.world();
    const aanvraag = world.actions.find((a) => a.id === 'burger.aanvraag');
    expect(aanvraag, 'geen actie burger.aanvraag').toBeTruthy();
    expect(aanvraag.prefill).toMatchObject({ bsn: BSN, jaar: 2024 });
  });

  test('F2: het formulier toont die voorgevulde waarde', async () => {
    await expect(forms.nth(0).locator('input[aria-label="Bsn"]')).toHaveValue(BSN);
  });

  test('F3: een datumparameter krijgt een datumveld', async () => {
    await expect(forms.nth(0).locator('nldd-date-field')).not.toHaveCount(0);
  });

  test('S1: de aanvraag levert twee grammen — bij de burger en bij de ontvanger', async () => {
    await forms.nth(0).locator('button[type=submit]').click();
    world = await s.worldWhen(
      (w) => grams(w, 'burger', 'aanvragen').length === 1 && grams(w, 'toeslagen', 'aanvragen').length === 1,
      'de aanvraag landt bij de burger én bij toeslagen',
    );
  });

  test('J1: het journaal noteert de aanvraag met actor en grammen', async () => {
    const entry = world.journal[world.journal.length - 1];
    expect(entry, 'leeg journaal').toBeTruthy();
    expect(JSON.stringify(entry.actor)).toMatch(/burger/);
    expect((entry.grams || entry.gram_refs || []).length).toBeGreaterThanOrEqual(1);
  });

  test('N1: een besluit vóór het feit faalt leesbaar en legt niets vast', async () => {
    earlyDecision = await s.api('POST', '/api/actions/toeslagen.toekenning', { bsn: BSN });
    expect(earlyDecision.status).toBeGreaterThanOrEqual(400);
    expect(earlyDecision.text).toMatch(/niets vast/);
    expect(grams(await s.world(), 'toeslagen', 'beschikkingen')).toHaveLength(0);
  });

  // Deze staat nog open: een besluit dat de wereld weigert komt er vandaag als
  // 500 uit, en een 500 zegt "dit programma is stuk" terwijl er niets stuk is —
  // de wereld staat er alleen niet naar. `fixme` en niet `skip`: de bewering
  // hoort te gelden, en zodra de foutafbeelding van de simulator die weigering
  // als 4xx doorgeeft, is dit woord weghalen het hele werk.
  test.fixme('N2: zo’n weigering is een 4xx en geen 500', async () => {
    expect(earlyDecision.status).toBeGreaterThanOrEqual(400);
    expect(earlyDecision.status).toBeLessThan(500);
  });

  test('D1: het decretogram is een BESCHIKKING met het bevoegd gezag uit de wet', async () => {
    await s.advance('01-03-2024');
    await s.fillByLabel(forms.nth(1), 'Bsn', BSN);
    await forms.nth(1).locator('button[type=submit]').click();
    world = await s.worldWhen(
      (w) => grams(w, 'toeslagen', 'beschikkingen').length === 1,
      'de toekenning legt een decretogram vast',
    );
    fields = grams(world, 'toeslagen', 'beschikkingen')[0].fields;
    expect(fields.legal_character?.value).toBe('BESCHIKKING');
    expect(fields.competent_authority?.value).toBe('Dienst Toeslagen');
  });

  test('D2: het decretogram noemt wie besloot', async () => {
    expect(fields.besloten_door?.value).toBeTruthy();
  });

  test('D3: een waarde van een andere cel draagt dat zij geaccepteerd is', async () => {
    expect(fields.toetsingsinkomen?.value).toBe(81000);
    expect(JSON.stringify(fields.toetsingsinkomen.origin)).toMatch(/accept/i);
  });

  test('D4: cross-law binnen de cel kost geen celgrens', async () => {
    const cell = world.cells.find((c) => c.id === 'toeslagen');
    expect(cell.laws).toContain('algemene_wet_inkomensafhankelijke_regelingen');
    expect(fields.heeft_recht_op_zorgtoeslag?.value).toBe(true);
    expect(world.crossings).toHaveLength(1);
  });

  test('D5: de wetsversie van het besluit ligt vast', async () => {
    expect(String(fields.regulation_valid_from?.value)).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  test('D6: de kroniekstanden staan met hash in het gram', async () => {
    expect(fields.chronicle_sources, 'geen chronicle_sources').toBeTruthy();
    expect(JSON.stringify(fields.chronicle_sources.value)).toMatch(/[0-9a-f]{64}/);
  });

  test('D7: het API-beeld draagt bewust geen receipt', async () => {
    // Het receipt zit in het gram in de kroniek — met wandkloktijd erin, en dat
    // hoort niet in een contract dat een browser leest. De inhoud ervan staat
    // vast in `cargo test -p regelrecht-simulator`.
    expect('receipt' in fields).toBe(false);
  });

  test('D8: een geaccepteerde waarde draagt bron-cel, moment en vrager', async () => {
    const origin = JSON.stringify(fields.toetsingsinkomen.origin);
    expect(origin).toMatch(/belastingdienst/);
    expect(origin).toMatch(/asked_by/);
  });

  test('X1: precies één cross-cel-vraag in het observatielog', async () => {
    expect(world.crossings).toHaveLength(1);
  });

  test('J2: het journaal boekt het besluit met een statusverandering', async () => {
    const entry = world.journal.filter((e) => kindIs(e, 'Besluit')).pop();
    expect(entry, 'geen besluitregel in het journaal').toBeTruthy();
    expect(hasDelta(entry), JSON.stringify(entry).slice(0, 300)).toBe(true);
  });

  test('J3: de cross-cel-vraag hangt onder dat besluit', async () => {
    const vragen = world.journal.filter((e) => kindIs(e, 'Vraag'));
    expect(vragen.length).toBeGreaterThanOrEqual(1);
    for (const vraag of vragen) expect(vraag.parent, JSON.stringify(vraag)).not.toBeNull();
  });

  test('J4: de UI toont de journaalregel van het besluit', async () => {
    const entry = world.journal.filter((e) => kindIs(e, 'Besluit')).pop();
    const regel = entry.description || entry.text;
    expect(regel, 'de besluitregel heeft geen tekst om te tonen').toBeTruthy();
    expect(await s.body()).toContain(regel);
  });

  test('A1: de actie meldt daarna "al besloten"', async () => {
    expect(await s.textAll()).toMatch(/al besloten op/i);
  });

  test('A2: een tweede besluit vraagt eerst om bevestiging', async () => {
    await s.fillByLabel(forms.nth(1), 'Bsn', BSN);
    await forms.nth(1).locator('button[type=submit]').click();
    await expect.poll(async () => /Toch besluiten/.test(await s.textAll())).toBe(true);
  });

  test('A3: na annuleren is er geen tweede decretogram', async () => {
    // De knop draagt haar tekst in een attribuut en niet als kindtekst: een
    // nldd-button rendert het label in zijn shadow root, dus zoeken op
    // zichtbare tekst vindt de knop niet.
    await page.locator('nldd-inline-dialog nldd-button[text="Annuleren"]').first().click();
    await expect.poll(async () => /Toch besluiten/.test(await s.textAll())).toBe(false);
    expect(grams(await s.world(), 'toeslagen', 'beschikkingen')).toHaveLength(1);
  });

  test('M1: een lexostatus op een eerder moment heeft niets vastgesteld', async () => {
    await s.tab('Lexostatus');
    const keuze = page.locator('select');
    await keuze.nth(0).selectOption('toeslagen');
    await page.waitForTimeout(200);
    await keuze.nth(1).selectOption('zorgtoeslagbeschikking');
    const parameters = page.locator('nldd-text-field input[type=text]');
    await parameters.nth(0).fill('zaakkenmerk');
    await parameters.nth(1).fill(`zorgtoeslag/${BSN}`);
    const moment = page.locator('nldd-date-field input[type=text]').first();
    await moment.fill('01-02-2024');
    await moment.press('Tab');
    await page.getByText('Vraag stellen').first().click();
    await expect.poll(async () => /Niets vastgesteld/i.test(await s.textAll())).toBe(true);
  });

  test('M2: op het besluitmoment is het wel vastgesteld', async () => {
    const moment = page.locator('nldd-date-field input[type=text]').first();
    await moment.fill('01-03-2024');
    await moment.press('Tab');
    await page.getByText('Vraag stellen').first().click();
    await expect.poll(async () => await s.body()).toMatch(/vastgesteld, geldig op 01-03-2024/i);
  });

  test('M3: een moment ná de klok wordt geweigerd', async () => {
    const antwoord = await s.api(
      'GET',
      `/api/cells/toeslagen/lexostatus/zorgtoeslagbeschikking?zaakkenmerk=zorgtoeslag/${BSN}&op_moment=2024-04-01`,
    );
    expect(antwoord.status, antwoord.text.slice(0, 120)).toBeGreaterThanOrEqual(400);
  });

  test('G1: het grammen-tabblad toont alle grammen', async () => {
    await s.tab('Grammen');
    world = await s.world();
    const totaal = world.cells.reduce(
      (n, cell) => n + cell.chronicles.reduce((m, k) => m + (k.grams || []).length, 0),
      0,
    );
    const zichtbaar = (await s.textAll()).match(/(\d+) van (\d+) grammen/);
    expect(zichtbaar, 'geen telling "n van m grammen" op het tabblad').toBeTruthy();
    expect([Number(zichtbaar[1]), Number(zichtbaar[2])]).toEqual([totaal, totaal]);
  });

  test('B1: de vier kwartaaltermijnen staan als executogram bij de betaler', async () => {
    await s.tab('Acties');
    await s.advance('31-12-2024');
    world = await s.worldWhen(
      (w) => grams(w, 'belastingdienst', 'betalingen').length === 4,
      'vier termijnen bij de belastingdienst',
    );
    for (const gram of grams(world, 'belastingdienst', 'betalingen')) {
      expect(JSON.stringify(gram.kind)).toMatch(/executogram/i);
    }
  });

  test('B2: de ontvangst is gemeld bij de besluitende cel', async () => {
    expect(grams(world, 'toeslagen', 'betalingen')).toHaveLength(4);
  });

  test('J5: elke termijn krijgt een journaalregel met statusverandering', async () => {
    const betalingen = world.journal.filter((e) => kindIs(e, 'Betaling'));
    expect(betalingen).toHaveLength(4);
    for (const entry of betalingen) expect(hasDelta(entry), JSON.stringify(entry)).toBe(true);
  });

  test('V1: de vaststelling legt een tweede decretogram vast', async () => {
    await s.fillByLabel(forms.nth(2), 'Bsn', BSN);
    await forms.nth(2).locator('button[type=submit]').click();
    world = await s.worldWhen(
      (w) => grams(w, 'toeslagen', 'beschikkingen').length === 2,
      'de vaststelling levert een tweede decretogram',
    );
  });

  test('I1: een instelling staat vast zodra er op besloten is', async () => {
    expect(Object.keys(world.locked_settings || {})).toContain('betalingsritme');
  });

  test('E1: een onbekende actie geeft 404 met de lijst die er wel is', async () => {
    const antwoord = await s.api('POST', '/api/actions/bestaat.niet', {});
    expect(antwoord.status).toBe(404);
    expect(antwoord.text).toMatch(/wel:/);
  });

  test('E2: de klok terugzetten geeft 409', async () => {
    const antwoord = await s.api('POST', '/api/advance', { until: '2024-01-01' });
    expect(antwoord.status, antwoord.text.slice(0, 120)).toBe(409);
  });

  test('E3: een verkeerde datum geeft 400 met een Nederlandse melding', async () => {
    const antwoord = await s.api('POST', '/api/actions/burger.aanvraag', {
      bsn: BSN,
      jaar: 2025,
      ondertekend_op: '01-01-2025',
    });
    expect(antwoord.status).toBe(400);
    expect(antwoord.text).toMatch(/geen datum/);
    // Geen dubbel prefix: de melding komt uit de actie en wordt niet nog eens
    // door de cel voorzien van haar eigen naam.
    expect(antwoord.text).not.toMatch(/burger\.burger/);
  });
});

test.describe('een tweede sessie', () => {
  /** @type {Awaited<ReturnType<typeof openSession>>} */
  let s;

  test.beforeAll(async ({ browser }) => {
    s = await openSession(browser);
  });

  test.afterAll(async () => {
    await s?.close();
  });

  test('E4: een tweede sessie is een eigen wereld', async () => {
    const world = await s.world();
    expect(world.clock).toBe('2024-01-01');
    expect(grams(world, 'burger', 'aanvragen')).toHaveLength(0);
  });

  test('E5: een gemiste termijn is een waarschuwing en geen blokkade', async () => {
    await s.advance('02-03-2024');
    const world = await s.worldWhen((w) => w.warnings.length >= 1, 'de gemiste termijn wordt gemeld');
    expect(world.warnings.length).toBeGreaterThanOrEqual(1);
  });

  test('E6: opnieuw beginnen zet de wereld terug', async () => {
    const antwoord = await s.api('POST', '/api/reset', {});
    expect(antwoord.status).toBe(200);
    const world = await s.world();
    expect(world.clock).toBe('2024-01-01');
    expect(world.warnings).toHaveLength(0);
  });
});
