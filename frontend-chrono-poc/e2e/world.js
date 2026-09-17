// De greep op een lopende testopstelling: één browsersessie is één wereld, en
// alles wat een check nodig heeft loopt via dit bestand. De specs blijven
// daarmee een opsomming van beweringen over de wereld, zonder selector- of
// wachtwerk ertussen.

import { expect } from '@playwright/test';

/**
 * De basis-URL van de opstelling.
 *
 * `playwright.config.js` zet `CHRONO_POC_E2E_BASE` zodra hij de poort gekozen
 * heeft (of `E2E_BASE` overgenomen heeft), en workers erven die variabele. Hier
 * opnieuw een poort kiezen zou een adres opleveren waar niets luistert, en een
 * vaste val-terug-poort zou de suite zonder een woord tegen de dev-server van
 * iemand anders aan zetten — een wereld waarin de checks iets heel anders meten
 * dan ze beweren. Ontbreekt de variabele, dan is er iets mis met de
 * doorgifte en zeggen we dat.
 */
export function baseUrl() {
  const base = process.env.CHRONO_POC_E2E_BASE || process.env.E2E_BASE;
  if (!base) {
    throw new Error(
      'CHRONO_POC_E2E_BASE is niet gezet: draai de suite via haar configuratie ' +
        '(`just chrono-poc-e2e`), die kiest de poort en geeft hem hierlangs door.',
    );
  }
  return base;
}

/** De grammen van één kroniek van één cel, of een lege lijst. */
export function grams(world, cell, stream) {
  const found = (world.cells || []).find((c) => c.id === cell);
  const chronicle = ((found || {}).chronicles || []).find((k) => k.stream === stream);
  return (chronicle || {}).grams || [];
}

/** Of de soort van een journaalregel `kind` bevat; de vorm verschilt per soort. */
export function kindIs(entry, kind) {
  return JSON.stringify(entry.kind || '')
    .toLowerCase()
    .includes(kind.toLowerCase());
}

/** Of een journaalregel een statusverandering draagt, in welke vorm dan ook. */
export function hasDelta(entry) {
  const raw = JSON.stringify(entry);
  return (
    raw.includes('→') ||
    (Array.isArray(entry.status_changes) && entry.status_changes.length > 0) ||
    (Array.isArray(entry.status) && entry.status.length > 0) ||
    (Array.isArray(entry.changes) && entry.changes.length > 0)
  );
}

/**
 * Open een sessie: een eigen browsercontext, dus een eigen wereld op de server.
 *
 * De cookie die de server uitdeelt hangt aan de context; twee contexten zijn
 * twee werelden die elkaar niet raken (zie `packages/chrono-poc-web`).
 *
 * Standaard op "Achter de schermen" (`#/wereld`): daar staan de cellen, de
 * bediening en de tijdlijn waar de meeste checks over gaan. Een wereld zonder
 * portaal toont die pagina op elk adres, dus dit werkt voor elk wereldbestand.
 */
export async function openSession(browser, path = '/#/wereld') {
  const context = await browser.newContext({
    baseURL: baseUrl(),
    viewport: { width: 1500, height: 1000 },
  });
  const page = await context.newPage();
  const pageErrors = [];
  page.on('pageerror', (e) => pageErrors.push(e.message));
  await page.goto(path, { waitUntil: 'networkidle' });
  await page.waitForTimeout(800);
  return session(context, page, pageErrors);
}

function session(context, page, pageErrors) {
  /**
   * Een API-aanroep vanuit de pagina zelf, en niet via `page.request`: de
   * wereld hangt aan de sessiecookie van deze context, en een verzoek dat
   * buiten de pagina om gaat zou een tweede wereld openen.
   */
  const api = async (method, url, body) =>
    await page.evaluate(
      async ([method, url, body]) => {
        const response = await fetch(url, {
          method,
          headers: { 'content-type': 'application/json' },
          body: body ? JSON.stringify(body) : undefined,
        });
        const text = await response.text();
        let json = null;
        try {
          json = JSON.parse(text);
        } catch {
          json = null;
        }
        return { status: response.status, json, text };
      },
      [method, url, body],
    );

  const world = async () => (await api('GET', '/api/world')).json;

  /** Klik een tabblad van de bediening aan. */
  const tab = async (name) => {
    await page.locator('nldd-tab-bar-item', { hasText: name }).first().click();
    await page.waitForTimeout(500);
  };

  /** De zichtbare tekst van de pagina. */
  const body = async () => await page.evaluate(() => document.body.innerText);

  /**
   * Alle tekst, inclusief wat in een shadow root van het ontwerpsysteem zit.
   *
   * De nldd-componenten dragen hun label in een attribuut (`text`, `label`,
   * `supporting-label`, …) of achter een shadow root, en `innerText` van de
   * pagina ziet daar niets van. Elke bewering over een melding die uit een
   * nldd-component komt moet dus hierlangs.
   */
  const textAll = async () =>
    await page.evaluate(() => {
      const out = [document.body.innerText];
      const walk = (root) => {
        for (const el of root.querySelectorAll('*')) {
          for (const attr of ['text', 'label', 'supporting-label', 'title', 'heading']) {
            const value = el.getAttribute && el.getAttribute(attr);
            if (value) out.push(value);
          }
          if (el.shadowRoot) {
            out.push(el.shadowRoot.textContent || '');
            walk(el.shadowRoot);
          }
        }
      };
      walk(document);
      return out.join('\n');
    });

  /** Zet de klok vooruit tot `date` (dd-mm-jjjj) en laat de triggers afgaan. */
  const advance = async (date) => {
    // Op het label van het veld en niet op "het laatste datumveld van de
    // pagina": een actie met een datumparameter draagt er ook een, en welke
    // van de twee de laatste is hangt aan de volgorde waarin de wereld haar
    // acties opsomt. Het label komt van de nldd-form-field eromheen terecht op
    // de `aria-label` van de input erin.
    const field = page.locator('input[aria-label="Spoel vooruit tot"]');
    await field.fill(date);
    await field.press('Tab');
    await page.getByRole('button', { name: 'Vooruitspoelen' }).click();
    await page.waitForTimeout(1200);
  };

  /**
   * Het formulier van één actie, gezocht op de actie-id die haar kaart toont.
   *
   * Niet op volgorde (`form` nummer twee): een actie erbij in het wereldbestand
   * verschuift die nummering, en dan vult een check stilzwijgend het verkeerde
   * formulier in.
   */
  const actionForm = (id) =>
    page.locator('nldd-card', { has: page.getByText(id, { exact: true }) }).locator('form');

  /** Vul een veld van een actieformulier op zijn label; ook checkboxen. */
  const fillByLabel = async (form, label, value) => {
    const field = form.locator(`input[aria-label="${label}"]`);
    if ((await field.getAttribute('type')) === 'checkbox') {
      if ((await field.isChecked()) !== (value === true)) await field.click();
    } else {
      await field.fill(String(value));
    }
  };

  /**
   * Wacht tot het wereldbeeld aan `predicate` voldoet en geef het terug.
   *
   * Een actie loopt via een besluit op de server; hoe lang dat duurt hangt aan
   * de machine, niet aan de wereld. Wachten op de uitkomst in plaats van op een
   * vast aantal milliseconden houdt de suite onder belasting overeind.
   */
  const worldWhen = async (predicate, message) => {
    await expect
      .poll(async () => predicate(await world()), { message, timeout: 20_000 })
      .toBe(true);
    return await world();
  };

  /**
   * Het formulier van het tabblad Lexostatus.
   *
   * Alles wat er staat komt uit de gekozen definitie, en de keuzelijsten van de
   * instellingen staan in dezelfde boom maar erbuiten; daarom telt alles
   * hieronder binnen dít formulier en niet "de eerste keuzelijst van de pagina".
   *
   * Gezocht op het veld dat alleen dit formulier heeft — het moment van de vraag
   * — en niet op "het eerste formulier van de pagina": de acties en de
   * instellingen hebben er ook een, en welk daarvan het eerste is hangt aan het
   * tabblad dat openstaat. Zo wacht een greep op een tabblad dat nog aan het
   * wisselen is, in plaats van stilzwijgend een ander formulier in te vullen.
   */
  const lexostatusForm = () =>
    page.locator('form', { has: page.locator('input[aria-label="Op moment"]') });

  /**
   * Het veld van één parameter, als de omringende `nldd-form-field`.
   *
   * Op het label van het invoerelement en niet op dat van het veld: het label
   * van een `nldd-form-field` staat als eigenschap en niet als attribuut, en
   * daar is geen selector op te leggen. Het invoerelement draagt zijn label wel
   * als attribuut, en dat is er één per parameter.
   */
  const lexostatusField = (param) =>
    lexostatusForm().locator('nldd-form-field', {
      has: page.locator(`[accessible-label="Waarde van ${param}"]`),
    });

  /** Wat het veld van deze parameter aan bekende waarden aanbiedt. */
  const lexostatusOptions = async (param) =>
    await lexostatusField(param)
      .locator('nldd-menu-item')
      .evaluateAll((items) => items.map((item) => item.getAttribute('text')));

  /**
   * Stel de vraag van het tabblad Lexostatus.
   *
   * Wat je weglaat blijft staan zoals het stond: dezelfde vraag op een ander
   * moment is één sleutel verzetten, niet het formulier opnieuw invullen.
   *
   * De parameters gaan er met echte toetsaanslagen in. Het veld van een sleutel
   * is een `nldd-combo-box`, en die werkt zijn waarde bij op wat er in zijn
   * eigen invoerveld gebeurt; `fill` zet die waarde er in één keer neer en het
   * component ziet er niets van. Na het typen staat de keuzelijst open, over de
   * knop heen — Escape doet hem dicht zonder de getypte waarde aan te raken.
   */
  const askLexostatus = async ({ cell, name, params, moment } = {}) => {
    const keuze = lexostatusForm().locator('select');
    if (cell) {
      await keuze.nth(0).selectOption(cell);
      await page.waitForTimeout(200);
    }
    if (name) {
      await keuze.nth(1).selectOption(name);
      await page.waitForTimeout(200);
    }
    for (const [param, value] of Object.entries(params || {})) {
      const veld = lexostatusField(param).locator('input');
      await veld.click();
      await veld.press('ControlOrMeta+a');
      // Wissen met een echte toets, en niet door er niets overheen te typen:
      // een lege waarde is een eigen vraag — die zonder deze parameter. Bleef
      // de selectie staan, dan ging die vraag stilzwijgend over wat er nog
      // stond, en een check die dat beweert zou groen zijn om het verkeerde.
      await veld.press('Delete');
      await veld.pressSequentially(String(value));
      await page.keyboard.press('Escape');
    }
    if (moment) {
      const veld = lexostatusForm().locator('input[aria-label="Op moment"]');
      await veld.fill(moment);
      await veld.press('Tab');
    }
    // De knop draagt haar tekst in een attribuut en niet als kindtekst (zie de
    // annuleerknop in de bewijsronde), en ze hoort bij dit formulier: op de hele
    // pagina zoeken vindt net zo goed een knop van een ander paneel.
    await lexostatusForm().locator('nldd-button[text="Vraag stellen"]').click();
  };

  return {
    context,
    page,
    pageErrors,
    api,
    world,
    worldWhen,
    tab,
    actionForm,
    body,
    textAll,
    advance,
    fillByLabel,
    lexostatusForm,
    lexostatusField,
    lexostatusOptions,
    askLexostatus,
    close: async () => await context.close(),
  };
}
