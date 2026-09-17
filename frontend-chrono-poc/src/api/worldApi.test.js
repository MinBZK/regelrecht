import { afterEach, describe, expect, it, vi } from 'vitest';
import { portaalFixture } from '../testing/portaalFixture.js';
import { worldFixture } from '../testing/worldFixture.js';
import {
  advanceTo,
  askLexostatus,
  choosePersona,
  fetchGramReceipt,
  fetchPortaal,
  isPortaal,
  isSnapshot,
  runAction,
  snapshotFrom,
  updateSettings,
} from './worldApi.js';

/** Eén nep-antwoord, genoeg voor apiFetch: ok, json en headers. */
function jsonResponse(body) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => 'application/json' },
    json: async () => body,
    text: async () => JSON.stringify(body),
  };
}

function stubFetch(body = worldFixture) {
  const fetchStub = vi.fn(async () => jsonResponse(body));
  vi.stubGlobal('fetch', fetchStub);
  return fetchStub;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('het beeld uit een antwoord', () => {
  it('herkent het beeld aan de klok en de cellen', () => {
    expect(isSnapshot(worldFixture)).toBe(true);
    expect(isSnapshot({ clock: '2025-01-01' })).toBe(false);
    expect(isSnapshot(null)).toBe(false);
  });

  it('neemt het beeld zoals het komt, los of onder een sleutel', () => {
    expect(snapshotFrom(worldFixture)).toBe(worldFixture);
    expect(snapshotFrom({ snapshot: worldFixture, events: [] })).toBe(worldFixture);
    expect(snapshotFrom({ world: worldFixture })).toBe(worldFixture);
  });

  it('geeft niets terug als er geen beeld in zit, zodat de aanroeper opnieuw ophaalt', () => {
    expect(snapshotFrom({ events: ['iets'] })).toBeNull();
    expect(snapshotFrom(undefined)).toBeNull();
  });
});

describe('de routes', () => {
  it('voert een actie uit met de velden als body', async () => {
    const fetchStub = stubFetch();
    await runAction('burger.aanvraag', { bsn: '999993653', jaar: 2025 });
    const [url, init] = fetchStub.mock.calls[0];
    expect(url).toBe('/api/actions/burger.aanvraag');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body)).toStrictEqual({ bsn: '999993653', jaar: 2025 });
  });

  it('spoelt vooruit tot een dag', async () => {
    const fetchStub = stubFetch();
    await advanceTo('2027-04-01');
    const [url, init] = fetchStub.mock.calls[0];
    expect(url).toBe('/api/advance');
    expect(JSON.parse(init.body)).toStrictEqual({ until: '2027-04-01' });
  });

  it('wijzigt instellingen met PUT', async () => {
    const fetchStub = stubFetch();
    await updateSettings({ betalingsritme: 'maand' });
    const [url, init] = fetchStub.mock.calls[0];
    expect(url).toBe('/api/settings');
    expect(init.method).toBe('PUT');
  });

  it('stelt een lexostatus-vraag met de parameters in de query', async () => {
    const fetchStub = stubFetch({ cell: 'belastingdienst', name: 'toetsingsinkomen', outcome: {} });
    await askLexostatus('belastingdienst', 'toetsingsinkomen', { bsn: '999993653', leeg: '' });
    expect(fetchStub.mock.calls[0][0]).toBe('/api/cells/belastingdienst/lexostatus/toetsingsinkomen?bsn=999993653');
  });

  it('zet het moment van de vraag als gereserveerde parameter in de query', async () => {
    const fetchStub = stubFetch({ cell: 'brp', name: 'partnerschap', outcome: {} });
    await askLexostatus('brp', 'partnerschap', { bsn: '999993653' }, '2024-06-01');
    expect(fetchStub.mock.calls[0][0]).toBe(
      '/api/cells/brp/lexostatus/partnerschap?bsn=999993653&op_moment=2024-06-01',
    );
  });

  it('laat het moment weg als er geen gekozen is; dan geldt de klok van de server', async () => {
    const fetchStub = stubFetch({ cell: 'brp', name: 'partnerschap', outcome: {} });
    await askLexostatus('brp', 'partnerschap', { bsn: '999993653' });
    expect(fetchStub.mock.calls[0][0]).toBe('/api/cells/brp/lexostatus/partnerschap?bsn=999993653');
  });

  it('zet de uitleg van de server in de fout, zonder de JSON eromheen', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: false,
        status: 409,
        headers: { get: () => 'application/json' },
        text: async () => JSON.stringify({ error: "er ligt nog geen aanvraag in cel 'toeslagen'" }),
      })),
    );
    const failure = await runAction('toeslagen.besluit', {}).catch((cause) => cause);
    expect(failure.message).toBe("er ligt nog geen aanvraag in cel 'toeslagen'");
    expect(failure.status).toBe(409);
  });

  it('zet geen pagina van een proxy in een melding, alleen de status', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: false,
        status: 502,
        headers: { get: () => 'text/html' },
        text: async () => '<html><body>Bad Gateway</body></html>',
      })),
    );
    const failure = await runAction('toeslagen.besluit', {}).catch((cause) => cause);
    expect(failure.message).toBe('HTTP 502');
  });

  it('laat een naam met een schuine streep heel', async () => {
    const fetchStub = stubFetch({ outcome: {} });
    await askLexostatus('cel/een', 'naam/twee', {});
    expect(fetchStub.mock.calls[0][0]).toBe('/api/cells/cel%2Feen/lexostatus/naam%2Ftwee');
  });

  it('vraagt het receipt van één gram op zijn plek in de kroniek', async () => {
    const fetchStub = stubFetch({ gram: {}, timestamp: {} });
    await fetchGramReceipt('toeslagen', 'beschikkingen', 0);
    expect(fetchStub.mock.calls[0][0]).toBe(
      '/api/cells/toeslagen/chronicles/beschikkingen/grams/0/receipt',
    );
    // Plek nul is een plek en geen ontbrekende waarde: de eerste vastlegging in
    // een kroniek is juist degene die het vaakst opgevraagd wordt.
    expect(fetchStub.mock.calls[0][0]).toContain('/grams/0/');
  });

  it('laat ook hier een naam met een schuine streep heel', async () => {
    const fetchStub = stubFetch({ gram: {}, timestamp: {} });
    await fetchGramReceipt('cel/een', 'stroom/twee', 3);
    expect(fetchStub.mock.calls[0][0]).toBe(
      '/api/cells/cel%2Feen/chronicles/stroom%2Ftwee/grams/3/receipt',
    );
  });
});

describe('het portaal', () => {
  it('herkent een portaal, en neemt al het andere als geen portaal', async () => {
    expect(isPortaal(portaalFixture)).toBe(true);
    expect(isPortaal(worldFixture)).toBe(false);
    expect(isPortaal(null)).toBe(false);

    const fetchStub = stubFetch(portaalFixture);
    expect(await fetchPortaal()).toBe(portaalFixture);
    expect(fetchStub).toHaveBeenCalledWith('/api/portaal', expect.anything());

    stubFetch(null);
    expect(await fetchPortaal()).toBeNull();
  });

  it('kiest een persona met PUT, en niemand met null', async () => {
    const fetchStub = stubFetch();
    await choosePersona('aanvrager-a');
    await choosePersona('');
    const [[url, init], [, leeg]] = fetchStub.mock.calls;
    expect(url).toBe('/api/persona');
    expect(init.method).toBe('PUT');
    expect(JSON.parse(init.body)).toStrictEqual({ id: 'aanvrager-a' });
    expect(JSON.parse(leeg.body)).toStrictEqual({ id: null });
  });
});
