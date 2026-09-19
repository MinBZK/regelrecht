import { describe, expect, it } from 'vitest';
import { GRAMMAR } from '@regelrecht/frontend-shared/gherkin';
import { FEATURE_KEYWORDS_NL, TRANSLATED_STEP_IDS, matchStep, renderStepNl } from './gherkinNl.js';

describe('renderStepNl', () => {
  it('translates the keyword and phrases a matched step in Dutch', () => {
    const step = { keyword: 'Given', text: 'the calculation date is "2025-01-01"' };
    expect(renderStepNl(step)).toEqual({ keyword: 'Gegeven', text: 'de peildatum is "2025-01-01"', matched: true });
  });

  it('renders the law-scoped data source step with source, law and key', () => {
    const step = { keyword: 'And', text: 'the following "RvIG" data with key "bsn" for law "wet_brp":' };
    const out = renderStepNl(step);
    expect(out.keyword).toBe('En');
    expect(out.text).toBe('de volgende gegevens van "RvIG" voor "wet_brp", gesleuteld op "bsn":');
  });

  it('renders assertions', () => {
    expect(renderStepNl({ keyword: 'Then', text: 'output "hoogte_toeslag" equals 165412' }).text).toBe(
      'is "hoogte_toeslag" gelijk aan 165412',
    );
    expect(renderStepNl({ keyword: 'And', text: 'output "is_verzekerde" is true' }).text).toBe('is "is_verzekerde" waar');
  });

  it('falls back to the English text for a step outside the grammar', () => {
    const out = renderStepNl({ keyword: 'When', text: 'something the grammar does not know' });
    expect(out).toEqual({ keyword: 'Als', text: 'something the grammar does not know', matched: false });
  });

  it('has a Dutch line for every core step of the grammar', () => {
    // A new core step without a template would show up in English on stage.
    for (const entry of GRAMMAR.filter((e) => e.tier === 'core')) {
      const english = entry.template(entry.argTypes.map((t) => (t === 'number' ? '1' : 'x')));
      expect(matchStep(english)?.entry.id).toBe(entry.id);
      expect(TRANSLATED_STEP_IDS.has(entry.id), entry.id).toBe(true);
      expect(renderStepNl({ keyword: 'Given', text: english }).matched).toBe(true);
    }
    expect(FEATURE_KEYWORDS_NL.Feature).toBe('Functionaliteit');
  });
});
