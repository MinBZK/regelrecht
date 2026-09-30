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

  describe('bijwerken', () => {
    // Zonder dit kon een variant alleen groeien: wie in zijn eigen variant
    // doorwerkte kreeg er bij elke keer bewaren een bijna-gelijke naast.
    it('overschrijft dezelfde variant in plaats van er een naast te zetten', async () => {
      const { bewaarVariant, werkVariantBij, browserVarianten, browserVariantYaml } = await laad();
      const v = bewaarVariant({ titel: 'Mijn variant', bestanden: [bestand('value: 2')] });

      const { variant: bij } = werkVariantBij(v.id, { bestanden: [bestand('value: 3')] });

      expect(bij.id).toBe(v.id);
      expect(browserVarianten()).toHaveLength(1);
      expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 3');
    });

    it('houdt de titel als er geen nieuwe wordt meegegeven', async () => {
      const { bewaarVariant, werkVariantBij } = await laad();
      const v = bewaarVariant({ titel: 'Blijft heten', bestanden: [bestand()] });

      expect(werkVariantBij(v.id, { bestanden: [bestand('value: 9')] }).variant.title).toBe('Blijft heten');
    });

    it('hernoemt als er wel een titel bij zit, met hetzelfde id', async () => {
      const { bewaarVariant, werkVariantBij } = await laad();
      const v = bewaarVariant({ titel: 'Oude naam', bestanden: [bestand()] });

      const { variant: bij } = werkVariantBij(v.id, { bestanden: [bestand()], titel: 'Nieuwe naam' });

      // Het id blijft: anders verwijst een bewaarde kolomkeuze naar niets meer.
      expect(bij.id).toBe(v.id);
      expect(bij.title).toBe('Nieuwe naam');
    });

    // Bijwerken overschrijft werk dat er al stond. Gaat het activeren daarna
    // mis, dan moet de vorige inhoud terug kunnen komen; anders blijft er een
    // variant staan die nooit meer laadt.
    it('zet de vorige inhoud terug met herstel()', async () => {
      const { bewaarVariant, werkVariantBij, browserVariantYaml, browserVariant } = await laad();
      const v = bewaarVariant({ titel: 'Herstel', bestanden: [bestand('value: 2')] });
      const gemaakt = browserVariant(v.id).gemaakt;

      const { herstel } = werkVariantBij(v.id, { bestanden: [bestand('value: 3')], titel: 'Andere naam' });
      expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 3');

      herstel();

      const na = browserVariant(v.id);
      expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 2');
      expect(na.titel).toBe('Herstel');
      expect(na.gemaakt).toBe(gemaakt);
      expect(na.bijgewerkt).toBeUndefined();
    });

    it('laat na herstel geen tweede variant achter', async () => {
      const { bewaarVariant, werkVariantBij, browserVarianten } = await laad();
      const v = bewaarVariant({ titel: 'Eén blijft', bestanden: [bestand()] });

      werkVariantBij(v.id, { bestanden: [bestand('value: 5')] }).herstel();

      expect(browserVarianten()).toHaveLength(1);
    });

    it('draagt `bijgewerkt` mee naar de vorm die de app leest', async () => {
      const { bewaarVariant, werkVariantBij, browserVarianten } = await laad();
      const v = bewaarVariant({ titel: 'Zichtbaar', bestanden: [bestand()] });

      // Vers bewaard: nog nooit bijgewerkt.
      expect(browserVarianten()[0].bijgewerkt).toBe(null);

      werkVariantBij(v.id, { bestanden: [bestand('value: 6')] });

      expect(browserVarianten()[0].bijgewerkt).toBeTruthy();
    });

    it('houdt `gemaakt` staan en noteert wanneer er bijgewerkt is', async () => {
      const { bewaarVariant, werkVariantBij, browserVariant } = await laad();
      const v = bewaarVariant({ titel: 'Tijden', bestanden: [bestand()] });
      const gemaakt = browserVariant(v.id).gemaakt;

      werkVariantBij(v.id, { bestanden: [bestand('value: 4')] });

      const na = browserVariant(v.id);
      expect(na.gemaakt).toBe(gemaakt);
      expect(na.bijgewerkt).toBeTruthy();
    });

    it('ververst de vingerafdruk, zodat drift van de nieuwe basis uitgaat', async () => {
      const { bewaarVariant, werkVariantBij, controleerDrift } = await laad();
      const v = bewaarVariant({ titel: 'Drift', bestanden: [bestand('value: 2', 'value: 1')] });

      // De wet is gewijzigd; opnieuw bewaren gebeurt op die nieuwe tekst.
      werkVariantBij(v.id, { bestanden: [bestand('value: 3', 'value: 1b')] });

      expect(controleerDrift(v.id, () => 'value: 1b').afgedreven).toBe(false);
      expect(controleerDrift(v.id, () => 'value: 1').afgedreven).toBe(true);
    });

    it('overleeft een ververs', async () => {
      const eerste = await laad();
      const v = eerste.bewaarVariant({ titel: 'Na ververs', bestanden: [bestand('value: 2')] });
      eerste.werkVariantBij(v.id, { bestanden: [bestand('value: 7')] });

      const tweede = await laad();
      expect(tweede.browserVarianten()).toHaveLength(1);
      expect(tweede.browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 7');
    });

    it('weigert een variant die hier niet staat', async () => {
      const { werkVariantBij } = await laad();

      expect(() => werkVariantBij('eigen-bestaat-niet', { bestanden: [bestand()] })).toThrow(/niet in deze browser/);
    });

    it('weigert een lege set bestanden', async () => {
      const { bewaarVariant, werkVariantBij } = await laad();
      const v = bewaarVariant({ titel: 'Leeg', bestanden: [bestand()] });

      expect(() => werkVariantBij(v.id, { bestanden: [] })).toThrow(/niets bewerkt/);
    });
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
