import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import NotenRapport from './NotenRapport.vue';
import { aggregeer } from '../lib/notenanalyse.js';

// DS-elementen compileren tot rauwe custom elements, dus hun zichtbare tekst
// staat in het `text`-attribuut en niet in textContent. Alle asserties hier
// lezen daarom attributen (zelfde aanpak als DocumentList.test.js).

const VOCAB = [{ id: 'missing-document', label: 'Document ontbreekt' }];

function noot({ motivation = 'commenting', tags = [], artikel = '1', exact = 'x' } = {}) {
  const body = [{ type: 'TextualBody', value: 'uitleg', purpose: 'commenting' }];
  for (const t of tags) body.push({ type: 'TextualBody', value: t, purpose: 'tagging' });
  return {
    type: 'Annotation',
    motivation,
    target: {
      source: 'regelrecht://wet',
      selector: { type: 'TextQuoteSelector', exact, prefix: '', suffix: '', hint: { article_number: artikel } },
    },
    body,
  };
}

function monteer(perWet, props = {}) {
  return mount(NotenRapport, {
    props: { rapport: aggregeer(perWet, VOCAB), ...props },
  });
}

/** Alle `text`-attributen van elementen die aan `sel` voldoen of erin zitten. */
function teksten(wrapper, sel) {
  return wrapper.findAll(sel).flatMap((el) => el.findAll('nldd-text-cell').map((c) => c.attributes('text')));
}

/** Elk `text`/`supporting-text`-attribuut op de pagina, als één doorzoekbare string. */
function paginaTekst(wrapper) {
  return wrapper
    .findAll('[text], [supporting-text]')
    .flatMap((el) => [el.attributes('text'), el.attributes('supporting-text')])
    .filter(Boolean)
    .join(' | ');
}

describe('NotenRapport', () => {
  it('toont de lege toestand zonder noten', () => {
    const w = monteer([]);
    expect(paginaTekst(w)).toContain('Nog geen notities in dit traject');
    expect(w.findAll('[data-test="wet"]')).toHaveLength(0);
  });

  it('toont een rij per soort en per wet', () => {
    const w = monteer([
      { lawId: 'wet_a', notes: [noot({ motivation: 'questioning' }), noot({ motivation: 'commenting' })], ankers: null },
      { lawId: 'wet_b', notes: [noot({ motivation: 'questioning' })], ankers: null },
    ]);
    // De rij toont het Nederlandse label, niet de W3C-sleutel.
    expect(teksten(w, '[data-test="soort"]')).toEqual(['vraag', '2', 'toelichting', '1']);
    expect(w.findAll('[data-test="wet"]')).toHaveLength(2);
  });

  // De ankerfouten-lijst is het enige blok dat om actie vraagt; hij moet
  // doorlinken naar het artikel, want daar wordt de noot gerepareerd.
  it('linkt elke ankerfout naar zijn artikel', () => {
    const w = monteer(
      [{ lawId: 'wet_a', notes: [noot({ artikel: '3', exact: 'weg' })], ankers: [{ status: 'orphaned' }] }],
      { hrefVoor: (lawId, artikel) => `/editor/${lawId}/${artikel}` },
    );
    const rijen = w.findAll('[data-test="ankerfout"]');
    expect(rijen).toHaveLength(1);
    expect(rijen[0].attributes('href')).toBe('/editor/wet_a/3');
    expect(teksten(w, '[data-test="ankerfout"]')).toContain('wet_a · artikel 3');
  });

  it('meldt geen ankerfouten als alles zijn tekst vindt', () => {
    const w = monteer([{ lawId: 'wet_a', notes: [noot()], ankers: [{ status: 'found' }] }]);
    expect(paginaTekst(w)).toContain('Elke notitie vindt haar tekst');
  });

  // Het onderscheid dat het hele blok zijn waarde geeft: "niet gemeten" mag
  // nooit als "in orde" lezen.
  it('meldt niet-gemeten wetten apart in plaats van als gezond', () => {
    const w = monteer([{ lawId: 'wet_a', notes: [noot()], ankers: null }]);
    expect(paginaTekst(w)).not.toContain('Elke notitie vindt haar tekst');
    const ongemeten = w.get('[data-test="ongemeten"]');
    expect(ongemeten.attributes('supporting-text')).toContain('wet_a');
  });

  it('markeert een tag buiten het vocabulaire', () => {
    const w = monteer([
      { lawId: 'wet_a', notes: [noot({ tags: ['verzonnen'] }), noot({ tags: ['missing-document'] })], ankers: null },
    ]);
    const tagRijen = w.findAll('[data-test="tag"]');
    const onbekend = tagRijen.find((r) => r.get('nldd-text-cell').attributes('text') === 'verzonnen');
    expect(onbekend.get('nldd-text-cell').attributes('supporting-text')).toBe('niet in het vocabulaire');
    // De bekende tag toont zijn label uit het vocabulaire, niet zijn id.
    expect(teksten(w, '[data-test="tag"]')).toContain('Document ontbreekt');
  });

  // --- doorklikken ---------------------------------------------------------

  it('toont elke notitie als eigen rij, met waar zij bij hoort', () => {
    const w = monteer([
      {
        lawId: 'wet_a',
        notes: [noot({ artikel: '3', exact: 'naar redelijkheid', motivation: 'questioning' })],
        ankers: null,
      },
    ]);
    const rijen = w.findAll('[data-test="notitie"]');
    expect(rijen).toHaveLength(1);
    const cellen = rijen[0].findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(cellen).toContain('wet_a · artikel 3');
    expect(cellen).toContain('vraag');
    expect(rijen[0].get('nldd-text-cell').attributes('supporting-text')).toBe('bij: naar redelijkheid');
  });

  it('linkt elke notitie naar haar artikel', () => {
    const w = monteer([{ lawId: 'wet_a', notes: [noot({ artikel: '7' })], ankers: null }], {
      hrefVoor: (lawId, artikel) => `/editor/${lawId}/${artikel}`,
    });
    expect(w.get('[data-test="notitie"]').attributes('href')).toBe('/editor/wet_a/7');
  });

  // Het dragende principe: een getal in de kaarten is een ingang naar de rijen
  // erachter, geen dood aantal.
  it('filtert de lijst op de aangeklikte soort', async () => {
    const w = monteer([
      {
        lawId: 'wet_a',
        notes: [
          noot({ motivation: 'questioning', artikel: '1' }),
          noot({ motivation: 'commenting', artikel: '2' }),
        ],
        ankers: null,
      },
    ]);
    expect(w.findAll('[data-test="notitie"]')).toHaveLength(2);

    const vraagRij = w
      .findAll('[data-test="soort"]')
      .find((r) => r.get('nldd-text-cell').attributes('text') === 'vraag');
    await vraagRij.trigger('click');

    expect(w.findAll('[data-test="notitie"]')).toHaveLength(1);
    expect(w.get('[data-test="filter"]').attributes('text')).toContain('vraag');
  });

  it('haalt de filter weg bij een tweede klik op dezelfde rij', async () => {
    const w = monteer([
      {
        lawId: 'wet_a',
        notes: [noot({ motivation: 'questioning' }), noot({ motivation: 'commenting' })],
        ankers: null,
      },
    ]);
    const rij = w.findAll('[data-test="soort"]')[0];
    await rij.trigger('click');
    expect(w.findAll('[data-test="notitie"]')).toHaveLength(1);
    await rij.trigger('click');
    expect(w.findAll('[data-test="notitie"]')).toHaveLength(2);
    expect(w.find('[data-test="filter"]').exists()).toBe(false);
  });

  it('filtert ook op een ambiguïteit-tag', async () => {
    const w = monteer([
      {
        lawId: 'wet_a',
        notes: [noot({ tags: ['missing-document'] }), noot({})],
        ankers: null,
      },
    ]);
    await w.get('[data-test="tag"]').trigger('click');
    expect(w.findAll('[data-test="notitie"]')).toHaveLength(1);
  });

  it('gebruikt de meegegeven naam voor een wet', () => {
    const w = monteer([{ lawId: 'wet_a', notes: [noot()], ankers: null }], {
      naamVoor: (id) => (id === 'wet_a' ? 'Wet A' : id),
    });
    expect(teksten(w, '[data-test="wet"]')).toContain('Wet A');
  });
});
