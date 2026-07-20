import { describe, it, expect } from 'vitest';
import { humanizeLawId } from './lawName.js';

describe('humanizeLawId', () => {
  it('turns a snake_case law id into title-cased words', () => {
    expect(humanizeLawId('algemene_wet_inkomensafhankelijke_regelingen')).toBe(
      'Algemene Wet Inkomensafhankelijke Regelingen',
    );
  });

  it('keeps digits and capitalises each word', () => {
    expect(humanizeLawId('burgerlijk_wetboek_boek_5')).toBe('Burgerlijk Wetboek Boek 5');
  });

  it('returns an empty string for nullish/empty input', () => {
    expect(humanizeLawId(null)).toBe('');
    expect(humanizeLawId(undefined)).toBe('');
    expect(humanizeLawId('')).toBe('');
  });
});
