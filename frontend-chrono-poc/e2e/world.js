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
 * opnieuw een poort kiezen zou een adres opleveren waar niets luistert.
 */
export const BASE = process.env.CHRONO_POC_E2E_BASE || process.env.E2E_BASE || 'http://localhost:7160';

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
 */
export async function openSession(browser) {
  const context = await browser.newContext({
    baseURL: BASE,
    viewport: { width: 1500, height: 1000 },
  });
  const page = await context.newPage();
  const pageErrors = [];
  page.on('pageerror', (e) => pageErrors.push(e.message));
  await page.goto('/', { waitUntil: 'networkidle' });
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
    const field = page.locator('nldd-date-field input[type=text]').last();
    await field.fill(date);
    await field.press('Tab');
    await page.getByRole('button', { name: 'Vooruitspoelen' }).click();
    await page.waitForTimeout(1200);
  };

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

  return {
    context,
    page,
    pageErrors,
    api,
    world,
    worldWhen,
    tab,
    body,
    textAll,
    advance,
    fillByLabel,
    close: async () => await context.close(),
  };
}
