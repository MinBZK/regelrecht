/**
 * Een wereld-API in geheugen voor de tests van de pagina's: `fetch` vervangen
 * door iets dat antwoordt zoals `packages/chrono-poc-web` dat doet, en dat
 * onthoudt welke persona er gekozen is.
 *
 * Alleen de routes die de pagina's van het portaal raken. Wat een test verder
 * nodig heeft, zet hij er zelf naast.
 */
import { vi } from 'vitest';
import { portaalFixture, worldAs } from './portaalFixture.js';

/** Het antwoord van de server, in de vorm die apiFetch verwacht. */
export function jsonResponse(body) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => 'application/json' },
    json: async () => body,
    text: async () => JSON.stringify(body),
  };
}

/**
 * Zet een nep-server neer en geef zijn verzoeken terug.
 *
 * `answers` is lexostatusnaam → antwoord; een naam die er niet in staat, krijgt
 * "niets vastgesteld" met een reden, net als bij een echte cel.
 */
export function fakeServer({ portaal = portaalFixture, persona = null, answers = {} } = {}) {
  let world = worldAs(persona);
  const requests = [];
  const fetch = vi.fn(async (url, init = {}) => {
    const method = init.method ?? 'GET';
    const [path, query = ''] = String(url).split('?');
    requests.push({ method, path, query: Object.fromEntries(new URLSearchParams(query)), body: init.body ? JSON.parse(init.body) : undefined });
    if (path === '/api/portaal') return jsonResponse(portaal);
    if (path === '/api/world') return jsonResponse(world);
    if (path === '/api/persona' && method === 'PUT') {
      world = worldAs(JSON.parse(init.body).id);
      return jsonResponse(world);
    }
    if (path.startsWith('/api/actions/') && method === 'POST') return jsonResponse({ snapshot: world, events: {} });
    const lexostatus = /^\/api\/cells\/([^/]+)\/lexostatus\/([^/]+)$/.exec(path);
    if (lexostatus) {
      const [, cell, name] = lexostatus.map(decodeURIComponent);
      return jsonResponse(
        answers[name] ?? {
          cell,
          name,
          op_moment: world.clock,
          outcome: { not_established: { reason: `in kroniek van cel '${cell}' ligt niets over deze zaak` } },
          reductie: null,
        },
      );
    }
    throw new Error(`de nep-server kent ${method} ${path} niet`);
  });
  vi.stubGlobal('fetch', fetch);
  return { fetch, requests };
}
