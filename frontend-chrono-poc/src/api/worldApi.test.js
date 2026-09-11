import { afterEach, describe, expect, it, vi } from 'vitest';
import { worldFixture } from '../testing/worldFixture.js';
import { advanceTo, askLexostatus, isSnapshot, runAction, snapshotFrom, updateSettings } from './worldApi.js';

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

  it('laat een naam met een schuine streep heel', async () => {
    const fetchStub = stubFetch({ outcome: {} });
    await askLexostatus('cel/een', 'naam/twee', {});
    expect(fetchStub.mock.calls[0][0]).toBe('/api/cells/cel%2Feen/lexostatus/naam%2Ftwee');
  });
});
