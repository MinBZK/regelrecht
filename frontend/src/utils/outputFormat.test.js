import { describe, it, expect } from 'vitest';
import { formatOutputValue, formatOutputValueParts } from './outputFormat.js';

// Intl renders the euro sign followed by a non-breaking space; matching on the
// digits keeps the assertions readable and independent of that whitespace.
const EUROS = /1\.500,00/;

describe('formatOutputValueParts', () => {
  it('gives a eurocent output its euro supporting text, whatever it is called', () => {
    expect(formatOutputValueParts(150000, 'eurocent')).toEqual({
      text: '150000',
      supportingText: expect.stringMatching(EUROS),
    });
  });

  it('leaves an integer without a unit unformatted, even when it is called a bedrag', () => {
    expect(formatOutputValueParts(150000, null)).toEqual({
      text: '150000',
      supportingText: '',
    });
  });

  it('formats a euro output without dividing', () => {
    const parts = formatOutputValueParts(1500, 'euro');
    expect(parts.text).toBe('1500');
    expect(parts.supportingText).toMatch(EUROS);
  });

  it('ignores units that are not money', () => {
    expect(formatOutputValueParts(65, 'jaar').supportingText).toBe('');
    expect(formatOutputValueParts(150000, 'percentage').supportingText).toBe('');
  });

  it('never currency-formats a non-numeric value', () => {
    expect(formatOutputValueParts('150000', 'eurocent').supportingText).toBe('');
    expect(formatOutputValueParts(true, 'eurocent')).toEqual({ text: 'ja', supportingText: '' });
    expect(formatOutputValueParts(null, 'eurocent')).toEqual({ text: 'null', supportingText: '' });
  });
});

describe('formatOutputValue', () => {
  it('appends the euro form for a eurocent value', () => {
    expect(formatOutputValue(150000, 'eurocent')).toMatch(/^150000 \(.*1\.500,00\)$/);
  });

  it('returns the bare value without a monetary unit', () => {
    expect(formatOutputValue(150000, null)).toBe('150000');
    expect(formatOutputValue(150000, 'eurocenten')).toBe('150000');
  });
});
