/**
 * De AI-keuze bij een werkdocument-upload, end-to-end door de echte editor.
 *
 * Twee dingen worden hier bewezen, precies de twee die een gebruiker merkt:
 *   - een markdown-bestand krijgt géén vinkje (er valt niets te kiezen: het
 *     wordt opgeslagen zoals het is);
 *   - een `.doc` — een formaat dat alleen een taalmodel kan lezen — houdt de
 *     uploadknop uit tot de gebruiker het vinkje aanzet, en stuurt dan pas
 *     iets, mét `llm=1` in de URL.
 *
 * Het bestand wordt rechtstreeks op het verborgen `<input type="file">` gezet.
 * Dat vuurt dezelfde `change` af als de bestandskiezer, zonder een native
 * dialoog te openen die Playwright anders moet onderscheppen.
 */
import { test, expect } from '@playwright/test';
import { mockAuthedEditor, TEST_TRAJECT_REF } from './helpers.js';

const REF = TEST_TRAJECT_REF;

// De indeling zoals editor-api 'm afleidt uit de convertertabel in de pipeline.
const UPLOAD_FORMATS = {
  passthrough: ['md', 'markdown'],
  deterministic: ['docx', 'odt', 'rtf', 'html', 'htm', 'epub', 'fb2', 'pdf'],
};

const json = (body) => ({
  status: 200,
  contentType: 'application/json',
  body: JSON.stringify(body),
});

async function gotoWerkdocumenten(page) {
  await mockAuthedEditor(page);
  await page.route('**/api/document-upload-formats', (r) => r.fulfill(json(UPLOAD_FORMATS)));
  // Lege randjes zodat de werkdocumenten-sectie rendert zonder echte backend.
  await page.route('**/api/sources', (r) => r.fulfill(json([])));
  await page.route('**/corpus/laws', (r) => r.fulfill(json([])));
  await page.route('**/corpus/changed-laws', (r) => r.fulfill(json([])));
  await page.route('**/corpus/documents', (r) => r.fulfill(json({ documents: [] })));
  await page.route('**/corpus/documents/jobs', (r) => r.fulfill(json({ jobs: [] })));
  await page.route('**/api/tasks**', (r) => r.fulfill(json({ tasks: [] })));
  await page.route('**/api/favorites', (r) => r.fulfill(json([])));

  await page.goto(`/trajecten/${REF}/werkdocumenten`);
  await page.locator('input[type="file"]').first().waitFor({ state: 'attached', timeout: 10_000 });
}

/** Kies een bestand zoals de bestandskiezer dat zou opleveren. */
async function pickFile(page, name, mimeType, contents = 'inhoud') {
  await page
    .locator('input[type="file"]')
    .first()
    .setInputFiles({ name, mimeType, buffer: Buffer.from(contents) });
  await page.locator('[data-testid="upload-confirm-submit"]').waitFor({ timeout: 5_000 });
}

const checkbox = (page) => page.locator('[data-testid="upload-confirm-llm"]');
const submit = (page) => page.locator('[data-testid="upload-confirm-submit"]');

test.describe('AI-keuze bij werkdocument-upload', () => {
  test('markdown toont geen vinkje', async ({ page }) => {
    await gotoWerkdocumenten(page);
    await pickFile(page, 'notitie.md', 'text/markdown', '# Titel\n');

    await expect(checkbox(page)).toHaveCount(0);
    await expect(submit(page)).not.toHaveAttribute('disabled', /.*/);
  });

  test('een .doc houdt de uploadknop uit tot het vinkje aan staat', async ({ page }) => {
    await gotoWerkdocumenten(page);

    let uploadUrl = null;
    await page.route('**/corpus/documents/upload**', (route, request) => {
      uploadUrl = request.url();
      return route.fulfill({
        status: 202,
        contentType: 'application/json',
        body: JSON.stringify({ target_path: 'brief.md' }),
      });
    });

    await pickFile(page, 'brief.doc', 'application/msword');

    await expect(checkbox(page)).toHaveCount(1);
    await expect(submit(page)).toHaveAttribute('disabled', /.*/);

    // Klikken terwijl de knop uit staat mag niets versturen.
    await submit(page).evaluate((el) => el.click());
    expect(uploadUrl).toBeNull();

    // Vinkje aan: de knop komt beschikbaar en de upload draagt de toestemming.
    await checkbox(page).evaluate((el) => {
      el.checked = true;
      el.dispatchEvent(new CustomEvent('change', { detail: { checked: true }, bubbles: true }));
    });
    await expect(submit(page)).not.toHaveAttribute('disabled', /.*/);

    await submit(page).evaluate((el) => el.click());
    await expect.poll(() => uploadUrl).not.toBeNull();
    expect(uploadUrl).toContain('llm=1');
  });
});
