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

// De drie acties van de publieke wereld, op id. De kaart van een actie draagt
// haar id zichtbaar; daarop zoeken houdt een check bij zijn eigen formulier,
// ook als de wereld er een actie bij krijgt.
const AANVRAAG = 'burger.aanvraag';
const TOEKENNING = 'toeslagen.toekenning';
const VASTSTELLING = 'toeslagen.vaststelling';

test.describe.configure({ mode: 'serial' });

test.describe('publieke wereld', () => {
  /** @type {Awaited<ReturnType<typeof openSession>>} */
  let s;
  let page;
  let world;
  let fields;
  let earlyDecision;

  test.beforeAll(async ({ browser }) => {
    s = await openSession(browser);
    page = s.page;
  });

  // Een uitzondering in de app laat de wereld-API ongemoeid, dus een check die
  // alleen naar het wereldbeeld kijkt blijft groen terwijl het scherm stuk is.
  // Daarom draagt elke stap ook deze bewering.
  test.afterEach(() => {
    expect(s.pageErrors).toEqual([]);
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

  // Het schema van een decretogram staat er vóórdat er iets gebeurd is: het is
  // de vorm die een besluit kan voortbrengen en geen samenvatting van wat er
  // ligt. Dat het hier in de browser gecontroleerd wordt en niet alleen in een
  // component-test, is omdat een ontwerpsysteem-component met een attribuut dat
  // het niet kent stil niets rendert — en dat blijkt pas hier.
  test('L4: het decretogram-schema van een besluit staat in de kolom van de cel', async () => {
    const lijst = page.locator('nldd-list[accessible-label="Decretogram-schema van cel toeslagen"]');
    await expect(lijst).toHaveCount(1);

    const rij = lijst
      .locator('nldd-list-item')
      .filter({ has: page.locator('nldd-text-cell[text="zorgtoeslag_toekenning"]') })
      .first();
    await rij.click();

    const tabel = page.locator('nldd-table[accessible-label*="zorgtoeslag_toekenning"]');
    await expect(tabel).toBeVisible();
    // Een veld dat de wet declareert, met het artikel erbij…
    await expect(
      tabel.locator('nldd-text-cell[supporting-text*="wet_op_de_zorgtoeslag, artikel 2"]').first(),
    ).toBeVisible();
    // …en een gat: normatieve inhoud die in het wereldbestand staat.
    await expect(tabel.locator('nldd-tag[text="gat"]').first()).toBeVisible();
  });

  test('F1: de aanvraag is voorgevuld uit het wereldbestand', async () => {
    world = await s.world();
    const aanvraag = world.actions.find((a) => a.id === 'burger.aanvraag');
    expect(aanvraag, 'geen actie burger.aanvraag').toBeTruthy();
    expect(aanvraag.prefill).toMatchObject({ bsn: BSN, jaar: 2024 });
  });

  test('F2: het formulier toont die voorgevulde waarde', async () => {
    await expect(s.actionForm(AANVRAAG).locator('input[aria-label="Bsn"]')).toHaveValue(BSN);
  });

  test('F3: een datumparameter krijgt een datumveld', async () => {
    await expect(s.actionForm(AANVRAAG).locator('nldd-date-field')).not.toHaveCount(0);
  });

  test('S1: de aanvraag levert twee grammen — bij de burger en bij de ontvanger', async () => {
    await s.actionForm(AANVRAAG).locator('button[type=submit]').click();
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

  // Een besluit dat de wereld weigert is geen kapot programma: de wereld staat
  // er alleen niet naar. Dat hoort de status ook te zeggen — 4xx en geen 500.
  test('N2: zo’n weigering is een 4xx en geen 500', async () => {
    expect(earlyDecision.status).toBeGreaterThanOrEqual(400);
    expect(earlyDecision.status).toBeLessThan(500);
  });

  test('D1: het decretogram is een BESCHIKKING met het bevoegd gezag uit de wet', async () => {
    await s.advance('01-03-2024');
    await s.fillByLabel(s.actionForm(TOEKENNING), 'Bsn', BSN);
    await s.actionForm(TOEKENNING).locator('button[type=submit]').click();
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
    // Het moment waarop de bron bevraagd is, en niet zomaar een moment: een
    // waarde die op de verkeerde dag is opgehaald is een ander feit.
    expect(origin).toMatch(/"op_moment":\s*"2024-03-01"/);
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
    // `?? null`: een veld dat verdwijnt is `undefined`, en dat glipt langs een
    // kale `not.toBeNull()` heen — precies de regressie die deze check zoekt.
    for (const vraag of vragen) expect(vraag.parent ?? null, JSON.stringify(vraag)).not.toBeNull();
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
    await s.fillByLabel(s.actionForm(TOEKENNING), 'Bsn', BSN);
    await s.actionForm(TOEKENNING).locator('button[type=submit]').click();
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
    await s.askLexostatus({
      cell: 'toeslagen',
      name: 'zorgtoeslagbeschikking',
      params: { zaakkenmerk: `zorgtoeslag/${BSN}` },
      moment: '01-02-2024',
    });
    await expect.poll(async () => /Niets vastgesteld/i.test(await s.textAll())).toBe(true);
  });

  test('M2: op het besluitmoment is het wel vastgesteld', async () => {
    await s.askLexostatus({ moment: '01-03-2024' });
    await expect.poll(async () => await s.body()).toMatch(/vastgesteld, geldig op 01-03-2024/i);
  });

  test('M3: een moment ná de klok wordt geweigerd', async () => {
    const antwoord = await s.api(
      'GET',
      `/api/cells/toeslagen/lexostatus/zorgtoeslagbeschikking?zaakkenmerk=zorgtoeslag/${BSN}&op_moment=2024-04-01`,
    );
    expect(antwoord.status, antwoord.text.slice(0, 120)).toBeGreaterThanOrEqual(400);
  });

  test('M4: het formulier legt uit waar het zaakkenmerk vandaan komt', async () => {
    // Het formulier komt uit de definitie: wat de vraag betekent staat erbij,
    // de sleutel draagt de vorm die de besluiten van deze cel eraan geven, en
    // de zaken die er al liggen zijn te kiezen. Zonder dat alles is het veld
    // een lege regel waar alleen iemand die de wereld kent iets in krijgt.

    // Op de toelichting van déze definitie en niet op "de eerste uitklap in het
    // formulier": een tweede uitklap erbij zou de check stil over iets anders
    // laten gaan.
    await expect(
      s.lexostatusForm().locator('nldd-inline-dialog[text="zorgtoeslagbeschikking"]'),
    ).toHaveAttribute('supporting-text', /wat deze cel over deze zaak besloten heeft/i);
    await expect(s.lexostatusField('zaakkenmerk')).toHaveAttribute(
      'supporting-label',
      /sleutel van kroniek 'beschikkingen'.*zorgtoeslag\/\{bsn\}/,
    );
    // `poll` en geen kale lezing: de keuzelijst is een momentopname van de DOM,
    // en die leest zichzelf niet opnieuw als het formulier nog hertekent.
    await expect
      .poll(async () => await s.lexostatusOptions('zaakkenmerk'))
      .toContain(`zorgtoeslag/${BSN}`);
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

  test('T1: het receipt van het besluit toont de uitvoeringstrace met haar artikelen', async () => {
    // De trace zit in het gram en komt via de receipt-route mee; hier gaat het
    // om wat een lezer in de browser krijgt: een boom met per stap de regeling
    // en het artikel waar ze vandaan komt.
    const rij = page
      .locator('nldd-list > nldd-list-item')
      .filter({ has: page.locator('nldd-text-cell[text="zorgtoeslag_toekenning"]') })
      .first();
    await rij.click();

    const receipt = rij
      .locator('nldd-list-item[slot="children"]')
      .filter({ has: page.locator('nldd-text-cell[text="Receipt"]') })
      .first();
    await receipt.click();

    const boom = rij.locator('nldd-list[type="tree"]').first();
    await expect(boom).toBeVisible();
    // De wortel staat open, dus de stappen eronder staan er; één ervan noemt het
    // artikel dat de hoogte uitrekent.
    await expect(
      boom.locator('nldd-text-cell[supporting-text*="wet_op_de_zorgtoeslag, artikel"]').first(),
    ).toBeVisible();
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

  test('T2: een betaling wijst terug naar het besluit waaruit ze volgt', async () => {
    // De verwijzing staat in het gram (cel, kroniek, plek, termijn); in het
    // scherm is ze de weg terug naar het besluit en dus naar zijn trace.
    await s.tab('Grammen');
    const rij = page
      .locator('nldd-list > nldd-list-item')
      .filter({ has: page.locator('nldd-text-cell[text="betaling_gedaan"]') })
      .first();
    await rij.click();

    const terug = rij
      .locator('nldd-list-item[slot="children"]')
      .filter({ has: page.locator('nldd-text-cell[text="Naar het besluit"]') })
      .first();
    await expect(terug).toBeVisible();
    await expect(terug.locator('nldd-text-cell[text="Naar het besluit"]')).toHaveAttribute(
      'supporting-text',
      /termijn \d/,
    );

    await terug.click();
    // De rij van het besluit gaat daardoor open: haar uitklap — met de weg naar
    // het receipt en dus naar de trace — staat er.
    const besluit = page
      .locator('nldd-list > nldd-list-item')
      .filter({ has: page.locator('nldd-text-cell[text="zorgtoeslag_toekenning"]') })
      .first();
    await expect(besluit.locator('nldd-text-cell[text="Receipt"]').first()).toBeVisible();

    // En terug naar het tabblad waar deze check hem vond: de ronde is één
    // verhaal, en de volgende stap vult een actieformulier in.
    await s.tab('Acties');
  });

  test('J5: elke termijn krijgt een journaalregel met statusverandering', async () => {
    const betalingen = world.journal.filter((e) => kindIs(e, 'Betaling'));
    expect(betalingen).toHaveLength(4);
    for (const entry of betalingen) expect(hasDelta(entry), JSON.stringify(entry)).toBe(true);
  });

  test('M5: openstaand toont de termijnen als tabel, en er staat niets open', async () => {
    // Na vier nagekomen termijnen is er niets open. Dat is het saaie antwoord,
    // en juist daarom het bewijs: de bedragen komen uit de termijnen eronder,
    // en die staan er allemaal, elk met haar stand.
    await s.tab('Lexostatus');
    await s.askLexostatus({
      cell: 'toeslagen',
      name: 'openstaande_termijnen',
      params: { zaakkenmerk: `zorgtoeslag/${BSN}` },
      moment: '31-12-2024',
    });

    const tabel = page.locator('nldd-table').first();
    await expect(tabel).toBeVisible();
    // De kop plus vier termijnen.
    await expect(tabel.locator('nldd-table-row')).toHaveCount(5);
    const kop = await tabel
      .locator('nldd-table-row')
      .first()
      .locator('nldd-text-cell')
      .evaluateAll((cells) => cells.map((cell) => cell.getAttribute('text')));
    expect(kop).toEqual(['Bedrag', 'Besluit', 'Status', 'Vervaldatum', 'Volgnummer']);
    // Vier nagekomen termijnen, dus vier keer dezelfde stand en geen enkele die
    // te laat is. De cellen dragen hun tekst in een attribuut (zoals overal in
    // het ontwerpsysteem), dus dit leest het attribuut en niet de bladzijde.
    await expect(tabel.locator('nldd-text-cell[text="betaald"]')).toHaveCount(4);
    await expect(tabel.locator('nldd-text-cell[text="te_laat"]')).toHaveCount(0);
    await s.tab('Acties');
  });

  test('V1: de vaststelling legt een tweede decretogram vast', async () => {
    await s.fillByLabel(s.actionForm(VASTSTELLING), 'Bsn', BSN);
    await s.actionForm(VASTSTELLING).locator('button[type=submit]').click();
    world = await s.worldWhen(
      (w) => grams(w, 'toeslagen', 'beschikkingen').length === 2,
      'de vaststelling levert een tweede decretogram',
    );
  });

  test('V2: de vaststelling herrekent op het definitieve inkomen en verrekent het voorschot', async () => {
    const vaststelling = grams(world, 'toeslagen', 'beschikkingen')[1].fields;
    expect(vaststelling.besluit?.value).toBe('zorgtoeslag_vaststelling');
    // Een ánder artikel dan de toekenning, uit een andere wet: de Awir stelt
    // vast, de Wet op de zorgtoeslag rekent de hoogte uit.
    expect(vaststelling.regulation?.value).toBe('algemene_wet_inkomensafhankelijke_regelingen');

    // Geen van beide invoerwaarden stelt deze uitvoering zelf vast: het inkomen
    // komt van de cel die de aanslag vaststelde, het uitbetaalde voorschot van
    // de cel die uitbetaalde.
    expect(JSON.stringify(vaststelling.toetsingsinkomen.origin)).toMatch(/belastingdienst/);
    expect(JSON.stringify(vaststelling.uitbetaalde_voorschotten.origin)).toMatch(
      /belastingdienst/,
    );

    // De herziene aanslag is hoger dan de voorlopige, dus de vastgestelde
    // tegemoetkoming valt lager uit dan het voorschot en het slotbedrag komt
    // onder nul. Dát is het verschil tussen vaststellen en overnemen wat er is
    // toegekend.
    expect(vaststelling.toetsingsinkomen.value).toBe(85000);
    expect(vaststelling.vastgestelde_tegemoetkoming.value).toBeLessThan(
      vaststelling.uitbetaalde_voorschotten.value,
    );
    expect(vaststelling.slotbedrag.value).toBeCloseTo(-75.16, 2);
  });

  test('X2: de vaststelling stelt haar eigen vragen over de celgrens', async () => {
    // Drie: de toekenning vroeg het toetsingsinkomen, de vaststelling vroeg het
    // opnieuw (de aanslag is inmiddels herzien) én vroeg wat er betaald is.
    expect(world.crossings).toHaveLength(3);
  });

  test('V3: een negatief slotbedrag wordt een terugvordering door de aanvrager', async () => {
    world = await s.worldWhen(
      (w) => grams(w, 'burger', 'betalingen').length === 1,
      'de terugvordering van de vaststelling is nagekomen',
    );
    // Een verplichting kent geen minteken, alleen een richting: de partijen
    // staan omgekeerd ten opzichte van de toekenning en het bedrag is positief.
    const terug = grams(world, 'burger', 'betalingen')[0].fields;
    expect(terug.soort.value).toBe('terugvordering');
    expect(terug.schuldenaar.value).toBe(BSN);
    expect(terug.schuldeiser.value).toBe('Dienst Toeslagen');
    expect(terug.bedrag.value).toBeCloseTo(75.16, 2);

    // En de betalende cel telt alleen op wat zíj betaalde: de vier
    // voorschottermijnen, samen precies het verleende bedrag. Wat de aanvrager
    // netto overhoudt is dat bedrag min de terugvordering — de vastgestelde
    // tegemoetkoming, tot op de cent.
    const voorschot = grams(world, 'belastingdienst', 'betalingen');
    expect(voorschot).toHaveLength(4);
    const betaald = voorschot.reduce((som, gram) => som + gram.fields.bedrag.value, 0);
    const verleend = grams(world, 'toeslagen', 'beschikkingen')[0].fields.hoogte_zorgtoeslag.value;
    expect(betaald).toBeCloseTo(verleend, 2);
    const vastgesteld =
      grams(world, 'toeslagen', 'beschikkingen')[1].fields.vastgestelde_tegemoetkoming.value;
    expect(betaald - terug.bedrag.value).toBeCloseTo(vastgesteld, 2);
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

  test.afterEach(() => {
    expect(s.pageErrors).toEqual([]);
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
