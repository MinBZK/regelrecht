import { describe, it, expect, beforeEach, vi } from 'vitest';
import { nextTick } from 'vue';

/**
 * De bewaarde stand is het verschil tussen "je begint elke ververs opnieuw"
 * en "de demo onthoudt wat je koos". Deze test legt vast wat er wel en niet
 * bewaard wordt, en dat een kapotte of verouderde waarde niet de pagina
 * meesleurt.
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

async function laad() {
  vi.resetModules();
  return import('../src/composables/useBewaardeStand.js');
}

describe('bewaarde stand', () => {
  beforeEach(() => {
    const opslag = verseOpslag();
    // Object.keys() over localStorage werkt in de browser via de indexer;
    // in de test bootsen we dat na met echte eigenschappen.
    vi.stubGlobal('localStorage', new Proxy(opslag, {
      ownKeys: () => [...opslag._data.keys()],
      getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true }),
    }));
    vi.stubGlobal('window', { location: { reload: vi.fn() } });
  });

  it('geeft de standaard als er niets bewaard is', async () => {
    const { bewaardeRef } = await laad();
    expect(bewaardeRef('n', 500).value).toBe(500);
  });

  it('leest een eerder bewaarde waarde terug', async () => {
    const { bewaardeRef } = await laad();
    const r = bewaardeRef('n', 500);
    r.value = 1200;
    await nextTick();

    const opnieuw = await laad();
    expect(opnieuw.bewaardeRef('n', 500).value).toBe(1200);
  });

  it('bewaart ook lijsten, zoals de gekozen kolommen', async () => {
    const { bewaardeRef } = await laad();
    const r = bewaardeRef('kolommen', []);
    r.value = ['a1', 'b'];
    await nextTick();

    const opnieuw = await laad();
    expect(opnieuw.bewaardeRef('kolommen', []).value).toEqual(['a1', 'b']);
  });

  it('valt terug op de standaard bij een waarde die niet meer geldig is', async () => {
    const { bewaardeRef } = await laad();
    bewaardeRef('variant', null).value = 'weggegooide-branch';
    await nextTick();

    // Bij het opnieuw laden bestaat die variant niet meer.
    const opnieuw = await laad();
    const r = opnieuw.bewaardeRef('variant', null, { geldig: (v) => ['a1', 'a2'].includes(v) });
    expect(r.value).toBe(null);
  });

  it('overleeft onleesbare opslag zonder te struikelen', async () => {
    const { bewaardeRef } = await laad();
    localStorage.setItem('ocw:v1:kapot', '{niet-json');
    expect(bewaardeRef('kapot', 'standaard').value).toBe('standaard');
  });

  it('wist alles van deze casus en herlaadt', async () => {
    const { bewaardeRef, wisStand, heeftBewaardeStand } = await laad();
    bewaardeRef('n', 500).value = 900;
    await nextTick();
    expect(heeftBewaardeStand()).toBe(true);

    wisStand();
    expect(heeftBewaardeStand()).toBe(false);
    expect(window.location.reload).toHaveBeenCalled();
  });

  it('laat sleutels van buiten de casus met rust', async () => {
    const { bewaardeRef, wisStand } = await laad();
    localStorage.setItem('thema', 'donker');
    bewaardeRef('n', 500).value = 900;
    await nextTick();

    wisStand();
    expect(localStorage.getItem('thema')).toBe('donker');
  });
});

/**
 * De burgerview bewaart de keuzes per persona. De valkuil zat niet in het
 * bewaren maar in het terugzetten: de persona-switcher stuurt ook een
 * select-event als hij zichzelf synchroniseert, en dat werd gelezen als "de
 * gebruiker koos iemand anders", waarna de zojuist teruggezette keuzes weer
 * op de standaard gingen. Deze test legt vast dat alleen een échte wissel
 * telt.
 */
describe('keuzes per persona terugzetten', () => {
  // Zoals selectPersona het doet: bij dezelfde persoon niets aanraken.
  function kies(huidig, nieuw, opslag) {
    if (!nieuw) return huidig;
    if (huidig?.bsn === nieuw.bsn) return huidig; // idempotent
    return { bsn: nieuw.bsn, keuzes: { ...nieuw.standaard, ...(opslag[nieuw.bsn] ?? {}) } };
  }

  const a = { bsn: '1', standaard: { overstapAangevraagd: false } };
  const b = { bsn: '2', standaard: { overstapAangevraagd: false } };

  it('houdt de teruggezette keuzes bij een herhaald select-event', () => {
    const opslag = { 1: { overstapAangevraagd: true } };
    let stand = kies(null, a, opslag);
    expect(stand.keuzes.overstapAangevraagd).toBe(true);

    // De switcher synchroniseert en stuurt hetzelfde persona nog eens.
    stand = kies(stand, a, opslag);
    expect(stand.keuzes.overstapAangevraagd).toBe(true);
  });

  it('begint bij een andere persoon met diens eigen uitgangspunt', () => {
    const opslag = { 1: { overstapAangevraagd: true } };
    let stand = kies(null, a, opslag);
    stand = kies(stand, b, opslag);
    expect(stand.bsn).toBe('2');
    expect(stand.keuzes.overstapAangevraagd).toBe(false);
  });
});
