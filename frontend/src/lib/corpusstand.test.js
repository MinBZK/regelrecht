import { describe, it, expect } from 'vitest';
import { aggregeer, tagWaarden, isOpen, artikelVanNoot } from './corpusstand.js';

// Minimale noot die aan het schema voldoet (required: type, motivation,
// target, body). De helpers eromheen houden de cases leesbaar.
function noot({ motivation = 'commenting', workflow, tags = [], artikel = '1', exact = 'x' } = {}) {
  const body = [{ type: 'TextualBody', value: 'uitleg', purpose: 'commenting' }];
  for (const t of tags) body.push({ type: 'TextualBody', value: t, purpose: 'tagging' });
  const n = {
    type: 'Annotation',
    motivation,
    target: {
      source: 'regelrecht://wet',
      selector: { type: 'TextQuoteSelector', exact, prefix: '', suffix: '', hint: { article_number: artikel } },
    },
    body,
  };
  if (workflow) n.workflow = workflow;
  return n;
}

const VOCAB = [
  { id: 'open-norm-not-filled', label: 'Open norm, nog niet ingevuld' },
  { id: 'missing-document', label: 'Document ontbreekt' },
];

describe('tagWaarden', () => {
  it('leest tagging-bodies uit een array', () => {
    expect(tagWaarden(noot({ tags: ['missing-document'] }))).toEqual(['missing-document']);
  });

  // De Rust-validator accepteert `body` ook als enkel object; wijken we daarvan
  // af, dan telt het dashboard andere tags dan CI valideert.
  it('leest een tagging-body die geen array is', () => {
    const n = { body: { type: 'TextualBody', value: 'open-norm-partial', purpose: 'tagging' } };
    expect(tagWaarden(n)).toEqual(['open-norm-partial']);
  });

  it('negeert bodies met een ander purpose', () => {
    expect(tagWaarden(noot())).toEqual([]);
  });

  it('valt niet om op een noot zonder body', () => {
    expect(tagWaarden({})).toEqual([]);
  });
});

describe('isOpen', () => {
  // Het schema geeft workflow de default `open`, dus een ontbrekend veld is
  // open — niet onbekend, en zeker niet afgehandeld.
  it('telt een ontbrekende workflow als open', () => {
    expect(isOpen(noot())).toBe(true);
  });

  it('telt resolved als niet-open', () => {
    expect(isOpen(noot({ workflow: 'resolved' }))).toBe(false);
  });
});

describe('aggregeer', () => {
  it('telt totalen en open noten over meerdere wetten', () => {
    const r = aggregeer(
      [
        { lawId: 'wet_a', notes: [noot(), noot({ workflow: 'resolved' })], ankers: null },
        { lawId: 'wet_b', notes: [noot()], ankers: null },
      ],
      VOCAB,
    );
    expect(r.totaal).toBe(3);
    expect(r.open).toBe(2);
    expect(r.wettenMetSidecar).toBe(2);
    expect(r.wettenMetNoten).toBe(2);
  });

  // Zonder het corpustotaal leest "noten over 2 wetten" als volledige
  // dekking. Alleen de aanroeper weet hoeveel wetten er zijn, dus het rapport
  // mag het niet uit `perWet` afleiden.
  it('draagt het corpustotaal door als de aanroeper het meegeeft', () => {
    const perWet = [{ lawId: 'wet_a', notes: [noot()], ankers: null }];
    expect(aggregeer(perWet, VOCAB).wettenInCorpus).toBe(null);
    expect(aggregeer(perWet, VOCAB, { wettenInCorpus: 24 }).wettenInCorpus).toBe(24);
  });

  it('groepeert op motivation, aantal aflopend', () => {
    const r = aggregeer(
      [
        {
          lawId: 'wet_a',
          notes: [noot({ motivation: 'questioning' }), noot({ motivation: 'questioning' }), noot({ motivation: 'commenting' })],
          ankers: null,
        },
      ],
      VOCAB,
    );
    expect(r.naarSoort).toEqual([
      { key: 'questioning', n: 2 },
      { key: 'commenting', n: 1 },
    ]);
  });

  it('markeert een tag die niet in het vocabulaire staat', () => {
    const r = aggregeer(
      [{ lawId: 'wet_a', notes: [noot({ tags: ['missing-document'] }), noot({ tags: ['verzonnen-tag'] })], ankers: null }],
      VOCAB,
    );
    const verzonnen = r.naarTag.find((t) => t.id === 'verzonnen-tag');
    expect(verzonnen.inVocabulaire).toBe(false);
    expect(verzonnen.label).toBe('verzonnen-tag'); // valt terug op de id
    expect(r.naarTag.find((t) => t.id === 'missing-document').label).toBe('Document ontbreekt');
    expect(r.buitenVocabulaire).toBe(1);
  });

  // De kern van deze metriek: de ankerstatus komt van de engine, nooit uit de
  // sidecar. Een `resolution: found` in het bestand mag nooit een orphaned
  // noot maskeren.
  it('neemt de ankerstatus van de engine, niet uit de sidecar', () => {
    const losgeraakt = noot({ artikel: '3', exact: 'weggevallen zinsnede' });
    losgeraakt.resolution = 'found'; // gecachete mening in het bestand
    const r = aggregeer([{ lawId: 'wet_a', notes: [losgeraakt], ankers: [{ status: 'orphaned' }] }], VOCAB);
    expect(r.ankerfouten.orphaned).toBe(1);
    expect(r.ankerfouten.items[0]).toMatchObject({ lawId: 'wet_a', artikel: '3', exact: 'weggevallen zinsnede' });
  });

  it('houdt niet-gemeten wetten apart van gezonde wetten', () => {
    const r = aggregeer([{ lawId: 'wet_a', notes: [noot()], ankers: null }], VOCAB);
    expect(r.ankerfouten.orphaned).toBe(0);
    expect(r.ongemeten).toEqual(['wet_a']);
  });

  // Bouwplan §5: gelijke invoer, gelijke uitvoer. De fan-out levert de wetten
  // in willekeurige volgorde aan, dus de aggregatie moet die volgorde wegnemen.
  it('geeft dezelfde uitvoer ongeacht de volgorde van binnenkomst', () => {
    const a = { lawId: 'wet_a', notes: [noot({ motivation: 'assessing' })], ankers: null };
    const b = { lawId: 'wet_b', notes: [noot({ motivation: 'assessing' })], ankers: null };
    expect(aggregeer([a, b], VOCAB)).toEqual(aggregeer([b, a], VOCAB));
  });

  it('sorteert ankerfouten numeriek op artikelnummer', () => {
    const r = aggregeer(
      [
        {
          lawId: 'wet_a',
          notes: [noot({ artikel: '10' }), noot({ artikel: '2' })],
          ankers: [{ status: 'orphaned' }, { status: 'orphaned' }],
        },
      ],
      VOCAB,
    );
    expect(r.ankerfouten.items.map((i) => i.artikel)).toEqual(['2', '10']);
  });

  it('geeft een leeg rapport zonder wetten', () => {
    const r = aggregeer([], VOCAB);
    expect(r.totaal).toBe(0);
    expect(r.naarSoort).toEqual([]);
    expect(r.ankerfouten.items).toEqual([]);
  });
});
