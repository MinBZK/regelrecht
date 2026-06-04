import { describe, it, expect } from 'vitest';
import { buildTypeMap } from './articleMapping.js';

describe('buildTypeMap', () => {
  it('maps parameter and input names to their declared type', () => {
    const articles = [
      {
        number: '2',
        machine_readable: {
          execution: {
            parameters: [{ name: 'bsn', type: 'string' }],
            input: [{ name: 'is_verzekerde', type: 'boolean' }],
          },
        },
      },
    ];
    const map = buildTypeMap(articles);
    expect(map.get('bsn')).toEqual({ type: 'string', unit: null });
    expect(map.get('is_verzekerde')).toEqual({ type: 'boolean', unit: null });
  });

  it('captures type_spec.unit for amounts', () => {
    const articles = [
      {
        number: '3',
        machine_readable: {
          execution: {
            parameters: [{ name: 'inkomen', type: 'amount', type_spec: { unit: 'eurocent' } }],
          },
        },
      },
    ];
    expect(buildTypeMap(articles).get('inkomen')).toEqual({ type: 'amount', unit: 'eurocent' });
  });

  it('lets parameter types override input types on name collision', () => {
    const articles = [
      {
        number: '1',
        machine_readable: {
          execution: {
            input: [{ name: 'x', type: 'string' }],
            parameters: [{ name: 'x', type: 'number' }],
          },
        },
      },
    ];
    expect(buildTypeMap(articles).get('x')).toEqual({ type: 'number', unit: null });
  });

  it('ignores articles without machine_readable and returns a Map', () => {
    const map = buildTypeMap([{ number: '1' }, {}]);
    expect(map).toBeInstanceOf(Map);
    expect(map.size).toBe(0);
  });

  it('handles empty / nullish input', () => {
    expect(buildTypeMap(undefined).size).toBe(0);
    expect(buildTypeMap([]).size).toBe(0);
  });
});
