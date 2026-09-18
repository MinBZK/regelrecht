import { describe, it, expect, beforeEach, vi } from 'vitest';

/**
 * Een browser-variant is het enige dat een gebruiker in deze demo maakt en
 * niet zo weer terugheeft. Deze test legt vast dat hij een ververs overleeft,
 * dat hij nooit een ingecheckte variant kan overschaduwen, en dat hij het
 * meldt als het corpus onder hem vandaan is gewijzigd.
 */
function verseOpslag() {
  const data = new Map();
  return {
    getItem: (k) => (data.has(k) ? data.get(k) : null),
    setItem: (k, v) => data.set(k, String(v)),
    removeItem: (k) => data.delete(k),
    get length() { return data.size; },
    key: (i) => [...data.keys()][i],
    _data: data,
  };
}

let opslag;

async function laad() {
  vi.resetModules();
  const stand = await import('./useBewaardeStand.js');
  stand.stelCasusIn('testcasus');
  return import('./browserVarianten.js');
}

describe('browser-varianten', () => {
  beforeEach(() => {
    opslag = verseOpslag();
    vi.stubGlobal('localStorage', new Proxy(opslag, {
      ownKeys: () => [...opslag._data.keys()],
      getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true }),
    }));
    vi.stubGlobal('window', { location: { reload: vi.fn() } });
  });

  const bestand = (yamlTekst = 'value: 2', origineel = 'value: 1') => ({
    pad: 'laws/wet/2025-01-01.yaml',
    yaml: yamlTekst,
    origineel,
  });

  it('bewaart een variant in de vorm die de lawStore kent', async () => {
    const { bewaarVariant } = await laad();

    const v = bewaarVariant({ titel: 'Voet naar 90%', bestanden: [bestand()] });

    expect(v).toMatchObject({
      id: 'eigen-voet-naar-90',
      title: 'Voet naar 90%',
      herkomst: 'browser',
      files: [{ path: 'laws/wet/2025-01-01.yaml', base: 'laws/wet/2025-01-01.yaml' }],
    });
  });

  it('geeft de bewaarde YAML terug per bestand', async () => {
    const { bewaarVariant, browserVariantYaml } = await laad();
    const v = bewaarVariant({ titel: 'Test', bestanden: [bestand('value: 42')] });

    expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 42');
    expect(browserVariantYaml(v.id, 'laws/andere.yaml')).toBe(null);
  });

  it('overleeft een ververs', async () => {
    const eerste = await laad();
    eerste.bewaarVariant({ titel: 'Blijft staan', bestanden: [bestand()] });

    // Nieuwe moduleinstantie, zelfde localStorage: dit is wat een ververs doet.
    const opnieuw = await laad();

    expect(opnieuw.browserVarianten()).toHaveLength(1);
    expect(opnieuw.browserVarianten()[0].title).toBe('Blijft staan');
  });

  it('blijft staan als de gebruiker opnieuw begint', async () => {
    const { bewaarVariant } = await laad();
    const stand = await import('./useBewaardeStand.js');
    bewaarVariant({ titel: 'Blijft', bestanden: [bestand()] });
    stand.bewaarStand('kolommen', ['a1']);

    stand.wisStand();

    const opnieuw = await laad();
    expect(opnieuw.browserVarianten()).toHaveLength(1);
  });

  it('nummert door bij een dubbele titel in plaats van te overschrijven', async () => {
    const { bewaarVariant, browserVarianten } = await laad();

    const a = bewaarVariant({ titel: 'Zelfde naam', bestanden: [bestand('a: 1')] });
    const b = bewaarVariant({ titel: 'Zelfde naam', bestanden: [bestand('b: 2')] });

    expect(a.id).toBe('eigen-zelfde-naam');
    expect(b.id).toBe('eigen-zelfde-naam-2');
    expect(browserVarianten()).toHaveLength(2);
  });

  // Een eigen variant die 'a1' heet zou de ingecheckte a1 uit variants.json
  // overschaduwen, en dan rekent de demo met iets anders dan de kolomkop zegt.
  it('kan een ingecheckte variant niet overschaduwen', async () => {
    const { bewaarVariant, isBrowserVariant } = await laad();

    const v = bewaarVariant({ titel: 'a1', bestanden: [bestand()], bestaandeIds: ['a1', 'a2'] });

    expect(v.id).not.toBe('a1');
    expect(isBrowserVariant(v.id)).toBe(true);
    expect(isBrowserVariant('a1')).toBe(false);
  });

  it('weigert een variant zonder titel of zonder bewerkingen', async () => {
    const { bewaarVariant } = await laad();

    expect(() => bewaarVariant({ titel: '  ', bestanden: [bestand()] })).toThrow(/titel/i);
    expect(() => bewaarVariant({ titel: 'Leeg', bestanden: [] })).toThrow(/niets bewerkt/i);
  });

  it('verwijdert een variant', async () => {
    const { bewaarVariant, verwijderVariant, browserVarianten } = await laad();
    const v = bewaarVariant({ titel: 'Weg ermee', bestanden: [bestand()] });

    expect(verwijderVariant(v.id)).toBe(true);
    expect(browserVarianten()).toHaveLength(0);
    expect(verwijderVariant(v.id)).toBe(false);
  });

  describe('drift', () => {
    it('meldt niets zolang het corpus gelijk blijft', async () => {
      const { bewaarVariant, controleerDrift } = await laad();
      const v = bewaarVariant({ titel: 'Stabiel', bestanden: [bestand('value: 2', 'value: 1')] });

      expect(controleerDrift(v.id, () => 'value: 1')).toEqual({
        afgedreven: false, paden: [], onbekend: false,
      });
    });

    // De wet is onder de variant vandaan gewijzigd: de bewerking is gemaakt op
    // een tekst die er niet meer zo staat.
    it('meldt het bestand waarvan de basis is gewijzigd', async () => {
      const { bewaarVariant, controleerDrift } = await laad();
      const v = bewaarVariant({ titel: 'Afgedreven', bestanden: [bestand('value: 2', 'value: 1')] });

      const uitkomst = controleerDrift(v.id, () => 'value: 999');

      expect(uitkomst.afgedreven).toBe(true);
      expect(uitkomst.paden).toEqual(['laws/wet/2025-01-01.yaml']);
    });

    // Een variant van voor deze controle draagt geen origineel mee. Dan valt
    // er niets te vergelijken en zegt de app liever niets dan iets onwaars.
    it('zegt onbekend als de variant geen uitgangspunt meedraagt', async () => {
      const { bewaarVariant, controleerDrift } = await laad();
      const v = bewaarVariant({
        titel: 'Oud',
        bestanden: [{ pad: 'laws/wet/2025-01-01.yaml', yaml: 'value: 2' }],
      });

      const uitkomst = controleerDrift(v.id, () => 'value: 999');

      expect(uitkomst).toEqual({ afgedreven: false, paden: [], onbekend: true });
    });

    it('slaat een bestand over dat de app nu niet kent', async () => {
      const { bewaarVariant, controleerDrift } = await laad();
      const v = bewaarVariant({ titel: 'Verdwenen', bestanden: [bestand()] });

      expect(controleerDrift(v.id, () => undefined).afgedreven).toBe(false);
    });

    // De basistekst wordt niet nog een keer bewaard: een wet is hier ~50 KB en
    // de opslag gaat rond de 5 MB dicht.
    it('bewaart een vingerafdruk en niet de hele basistekst', async () => {
      const { bewaarVariant, browserVariant } = await laad();
      const groot = `x: ${'y'.repeat(5000)}`;
      const v = bewaarVariant({
        titel: 'Groot',
        bestanden: [{ pad: 'laws/a.yaml', yaml: 'bewerkt', origineel: groot }],
      });

      const bewaard = browserVariant(v.id).bestanden[0];
      expect(bewaard.origineel).toBeUndefined();
      expect(bewaard.origineelHash.length).toBeLessThan(20);
    });
  });

  describe('vingerafdruk', () => {
    it('verschilt zodra de tekst verandert', async () => {
      const { vingerafdruk } = await laad();
      expect(vingerafdruk('value: 1')).toBe(vingerafdruk('value: 1'));
      expect(vingerafdruk('value: 1')).not.toBe(vingerafdruk('value: 2'));
      expect(vingerafdruk('ab')).not.toBe(vingerafdruk('ba'));
      expect(vingerafdruk(null)).toBe(null);
    });
  });

  // Een "bewaard" dat na een ververs weg blijkt te zijn, is erger dan een
  // weigering: de gebruiker denkt dan dat zijn werk veilig staat.
  it('meldt het hard als de opslag vol zit in plaats van stil te falen', async () => {
    const { bewaarVariant, browserVarianten } = await laad();
    const echteSet = opslag.setItem;
    opslag.setItem = () => { throw new DOMException('vol', 'QuotaExceededError'); };

    try {
      expect(() => bewaarVariant({ titel: 'Past niet', bestanden: [bestand()] }))
        .toThrow(/opslag zit vol|staat uit/i);
    } finally {
      opslag.setItem = echteSet;
    }

    expect(browserVarianten()).toHaveLength(0);
  });
});
