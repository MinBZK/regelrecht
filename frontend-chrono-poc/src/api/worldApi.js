/**
 * De wereld-API, zoals deze app hem gebruikt.
 *
 * Eén plek met de routes en één vorm voor het antwoord. De server geeft bij een
 * wijziging het nieuwe beeld terug plus wat er gebeurde; wat er gebeurde is van
 * de server (en van zijn versie), maar het **beeld** is het contract dat ook in
 * `packages/simulator` vastligt. Deze module pelt daarom alleen het beeld eruit
 * en valt terug op een verse `GET /api/world` als het antwoord er geen draagt.
 * Wat er nieuw is, leidt de app af uit twee opeenvolgende beelden (zie
 * `world/snapshot.js`) en niet uit een gebeurtenissenlijst, zodat deze frontend
 * niet op een tweede, losse vorm gaat leunen.
 *
 * Fouten komen door als `ApiError` met de leesbare Nederlandse tekst van de
 * server als `message`; zie `serverMessage`.
 */
import { apiFetchJson } from '@regelrecht/frontend-shared';

const JSON_HEADERS = { 'Content-Type': 'application/json' };

/**
 * De melding die een bezoeker ziet als de server het verzoek weigert.
 *
 * De server antwoordt met `{"error": "<uitleg van de wereld>"}`, en die uitleg is
 * wat in beeld hoort — niet de JSON eromheen. Komt het antwoord niet van de
 * server (een reverse proxy met een 502-pagina bijvoorbeeld), dan komt er alleen
 * de status: liever een kale status dan een pagina HTML in een melding. Dezelfde
 * afweging als bij `errorMessage` in `frontend/src/composables/useLaw.js`.
 */
function serverMessage(status, body, contentType) {
  if (contentType.startsWith('application/json') && body) {
    try {
      const parsed = JSON.parse(body);
      if (typeof parsed?.error === 'string' && parsed.error) return parsed.error;
    } catch {
      // Geen leesbare JSON: dan is de status het enige wat zeker klopt.
    }
  }
  return `HTTP ${status}`;
}

/** Eén verzoek aan de wereld-API: overal dezelfde foutmelding. */
function request(path, init = {}) {
  return apiFetchJson(path, { errorMessage: serverMessage, ...init });
}

/** Lijkt dit op een beeld van de wereld? Klok en cellen maken het beeld. */
export function isSnapshot(value) {
  return (
    value !== null
    && typeof value === 'object'
    && typeof value.clock === 'string'
    && Array.isArray(value.cells)
  );
}

/**
 * Het beeld uit een antwoord, of `null` als er geen in zit.
 *
 * Een antwoord op een wijziging mag het beeld zelf zijn of het onder `snapshot`
 * of `world` dragen; het is elke keer hetzelfde beeld en de aanroeper hoort het
 * verschil niet te kennen.
 */
export function snapshotFrom(payload) {
  if (isSnapshot(payload)) return payload;
  for (const key of ['snapshot', 'world']) {
    if (isSnapshot(payload?.[key])) return payload[key];
  }
  return null;
}

/** Het beeld van de wereld. */
export function fetchWorld() {
  return request('/api/world');
}

/** Voer een actie uit, met de ingevulde velden als body. */
export function runAction(id, values) {
  return request(`/api/actions/${encodeURIComponent(id)}`, {
    method: 'POST',
    headers: JSON_HEADERS,
    body: JSON.stringify(values ?? {}),
  });
}

/** Spoel de klok vooruit tot en met een dag. */
export function advanceTo(until) {
  return request('/api/advance', {
    method: 'POST',
    headers: JSON_HEADERS,
    body: JSON.stringify({ until }),
  });
}

/** Wijzig instellingen uit het wereldbestand. */
export function updateSettings(changes) {
  return request('/api/settings', {
    method: 'PUT',
    headers: JSON_HEADERS,
    body: JSON.stringify(changes),
  });
}

/** Zet de wereld terug naar zijn startstand. */
export function resetWorld() {
  return request('/api/reset', { method: 'POST' });
}

/**
 * Vraag een cel naar een lexostatus. Alleen lezen.
 *
 * "In deze cel is hierover niets vastgesteld" is een gewoon antwoord met status
 * 200, dus hier hoeft niets opgevangen te worden: het staat in de uitkomst.
 */
export function askLexostatus(cell, name, params = {}) {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (key && value !== '' && value !== null && value !== undefined) query.set(key, String(value));
  }
  const search = query.toString();
  const suffix = search ? `?${search}` : '';
  const path = `/api/cells/${encodeURIComponent(cell)}/lexostatus/${encodeURIComponent(name)}`;
  return request(`${path}${suffix}`);
}
