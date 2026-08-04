import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import CorpusstandReport from './CorpusstandReport.vue';
import { aggregeer } from '../lib/corpusstand.js';

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
  return mount(CorpusstandReport, {
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

describe('CorpusstandReport', () => {
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
    expect(teksten(w, '[data-test="soort"]')).toEqual(['questioning', '2', 'commenting', '1']);
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
    expect(paginaTekst(w)).toContain('Alle noten vinden hun tekst');
  });

  // Het onderscheid dat het hele blok zijn waarde geeft: "niet gemeten" mag
  // nooit als "in orde" lezen.
  it('meldt niet-gemeten wetten apart in plaats van als gezond', () => {
    const w = monteer([{ lawId: 'wet_a', notes: [noot()], ankers: null }]);
    expect(paginaTekst(w)).not.toContain('Alle noten vinden hun tekst');
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

  it('gebruikt de meegegeven naam voor een wet', () => {
    const w = monteer([{ lawId: 'wet_a', notes: [noot()], ankers: null }], {
      naamVoor: (id) => (id === 'wet_a' ? 'Wet A' : id),
    });
    expect(teksten(w, '[data-test="wet"]')).toContain('Wet A');
  });
});
