import { describe, it, expect } from 'vitest';
import { graafUitMetrieken } from './graafUitMetrieken.js';

function regeling(law_id, over = {}) {
  return {
    law_id,
    valid_from: '2025-01-01',
    valid_to: null,
    layer: 'WET',
    schema_version: 'v0.5.6',
    article_count: 3,
    articles_with_logic: 1,
    output_count: 2,
    incoming_bindings: 0,
    outgoing_bindings: 0,
    loaded: true,
    ...over,
  };
}

function binding(from_law, to_law, over = {}) {
  return { from_law, from_article: '1', to_law, label: 'bedrag', kind: 'source', integrity: 'clean', ...over };
}

function rapport({ regulations = [], bindings = [], totals = {} } = {}) {
  return { regulations, bindings, totals: { bindings_clean: 0, findings_by_class: {}, ...totals } };
}

describe('graafUitMetrieken', () => {
  it('maakt één knoop per regeling en één rand per binding', () => {
    const g = graafUitMetrieken(
      rapport({
        regulations: [regeling('wet_a'), regeling('wet_b', { incoming_bindings: 1 })],
        bindings: [binding('wet_a', 'wet_b')],
      }),
    );
    expect(g.knopen.map((k) => k.lawId)).toEqual(['wet_a', 'wet_b']);
    expect(g.randen).toEqual([
      { van: 'wet_a', naar: 'wet_b', soort: 'source', integriteit: 'clean', label: 'bedrag' },
    ]);
  });

  // Het rapport heeft een rij per versie. Die één op één overnemen levert
  // dubbele knoop-ids op, en dan tekent Vue Flow er willekeurig één.
  it('vouwt versies van dezelfde wet samen tot één knoop', () => {
    const g = graafUitMetrieken(
      rapport({
        regulations: [
          regeling('wet_a', { valid_from: '2024-01-01', article_count: 2, layer: 'AMVB' }),
          regeling('wet_a', { valid_from: '2026-01-01', article_count: 9, layer: 'WET' }),
        ],
      }),
    );
    expect(g.knopen).toHaveLength(1);
    // Metadata van de nieuwste versie.
    expect(g.knopen[0]).toMatchObject({ validFrom: '2026-01-01', artikelen: 9, laag: 'WET' });
  });

  // Optellen zou een wet met drie versies drie keer zo groot laten lijken.
  it('telt artikelen niet op over versies', () => {
    const g = graafUitMetrieken(
      rapport({
        regulations: [
          regeling('wet_a', { valid_from: '2024-01-01', article_count: 5 }),
          regeling('wet_a', { valid_from: '2025-01-01', article_count: 5 }),
        ],
      }),
    );
    expect(g.knopen[0].artikelen).toBe(5);
  });

  // Inkomende bindingen tellen wél op: een wet is aangeroepen zodra één van
  // haar versies wordt aangeroepen.
  it('noemt een wet aangeroepen als een van haar versies dat is', () => {
    const g = graafUitMetrieken(
      rapport({
        regulations: [
          regeling('wet_a', { valid_from: '2024-01-01', incoming_bindings: 0 }),
          regeling('wet_a', { valid_from: '2025-01-01', incoming_bindings: 2 }),
        ],
      }),
    );
    expect(g.knopen[0].inkomend).toBe(2);
    expect(g.knopen[0].nietAangeroepen).toBe(false);
  });

  it('markeert een regeling zonder enige inkomende binding als niet-aangeroepen', () => {
    const g = graafUitMetrieken(rapport({ regulations: [regeling('wet_los')] }));
    expect(g.knopen[0].nietAangeroepen).toBe(true);
  });

  // Een doelwet die niet geladen is, is zelf de bevinding en moet zichtbaar
  // blijven in plaats van als gezonde knoop te lezen.
  it('houdt een niet-geladen doelwet herkenbaar', () => {
    const g = graafUitMetrieken(
      rapport({ regulations: [regeling('wet_a'), regeling('wet_weg', { loaded: false })] }),
    );
    const weg = g.knopen.find((k) => k.lawId === 'wet_weg');
    expect(weg.aanwezig).toBe(false);
    expect(weg.nietAangeroepen).toBe(false);
  });

  it('neemt soort en integriteit ongewijzigd over uit het rapport', () => {
    const g = graafUitMetrieken(
      rapport({
        bindings: [binding('a', 'b', { kind: 'implements', integrity: 'impl-dangling', label: 'tarief' })],
      }),
    );
    expect(g.randen[0]).toMatchObject({ soort: 'implements', integriteit: 'impl-dangling', label: 'tarief' });
  });

  it('leidt de telling af uit de totalen van het rapport', () => {
    const g = graafUitMetrieken(
      rapport({ totals: { bindings_clean: 23, findings_by_class: { dangling: 2, 'impl-no-date': 1 } } }),
    );
    expect(g.telling).toMatchObject({ clean: 23, dangling: 2, 'impl-no-date': 1, misplaced: 0 });
  });

  it('valt niet om op een leeg of ontbrekend rapport', () => {
    expect(graafUitMetrieken(null).knopen).toEqual([]);
    expect(graafUitMetrieken(rapport()).randen).toEqual([]);
  });
});
