import { describe, expect, it } from 'vitest';
import { HINTS, HINT_KEYS, hintSegments } from './keyHints.js';
import { LOCALES } from '../i18n/index.js';

describe('hintSegments', () => {
  it('splits a sentence into keys and the text between them', () => {
    expect(hintSegments('{prev} {next} of {space} bladeren')).toEqual([
      { key: '←' },
      { text: ' ' },
      { key: '→' },
      { text: ' of ' },
      { key: 'Space' },
      { text: ' bladeren' },
    ]);
  });

  it('keeps a key wherever the language puts it', () => {
    expect(hintSegments('press {esc} to close')).toEqual([{ text: 'press ' }, { key: 'Esc' }, { text: ' to close' }]);
  });

  it('leaves an unknown placeholder visible instead of dropping it', () => {
    expect(hintSegments('{typo} sluit')).toEqual([{ text: '{typo} sluit' }]);
  });
});

describe('the hints under the slides', () => {
  // De toetsregel stond als Nederlandse tekst in de template, en bleef dus
  // Nederlands in het Engels en het Fries. Elke taal draagt nu elke hint, met
  // alleen toetsen die het dek kent, en met tekst naast de toetsen.
  it.each(LOCALES)('$code has every hint, with known keys and its own words', ({ dict }) => {
    for (const key of HINTS) {
      const sentence = dict[key];
      expect(sentence, key).toBeTypeOf('string');
      const names = [...sentence.matchAll(/\{(\w+)\}/g)].map((m) => m[1]);
      expect(names.filter((n) => !(n in HINT_KEYS)), key).toEqual([]);
      expect(hintSegments(sentence).some((s) => s.text?.trim()), key).toBe(true);
    }
  });
});
