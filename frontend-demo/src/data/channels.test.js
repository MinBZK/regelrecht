import { describe, expect, it } from 'vitest';
import { deliver, leaves, messageFor } from './channels.js';

const channels = [
  {
    from: { cell: 'a', event: 'opdracht' },
    to: { cell: 'b', article: 'voorwaarden#1' },
    inputs: { kenmerk: '$id', bedrag: 'bedrag', ontbreekt: 'nergens' },
  },
  {
    from: { cell: 'b', article: 'voorwaarden#1' },
    to: { cell: 'a', article: 'beleid#2', refers_to: { opdracht: 'kenmerk' } },
    inputs: { gelukt: 'gelukt' },
  },
];

describe('channels', () => {
  it('matches a gram by cell and event or article', () => {
    expect(leaves(channels[0], 'a', { name: 'opdracht' })).toBe(true);
    expect(leaves(channels[0], 'b', { name: 'opdracht' })).toBe(false);
    expect(leaves(channels[0], 'a', { name: 'anders' })).toBe(false);
    expect(leaves(channels[1], 'b', { establishes: 'voorwaarden#1' })).toBe(true);
    expect(leaves({ from: { cell: 'a' } }, 'a', { name: 'x' })).toBe(false);
  });

  it('maps fields to parameters with their provenance, and leaves out what is missing', () => {
    const { inputs, refersTo } = messageFor(channels[0], 'a', { id: 'g1', fields: { bedrag: 100 } });
    expect(inputs).toEqual({
      kenmerk: { value: 'g1', provenance: { source: 'kanaal', from: 'a', gram: 'g1', field: '$id' } },
      bedrag: { value: 100, provenance: { source: 'kanaal', from: 'a', gram: 'g1', field: 'bedrag' } },
    });
    expect(refersTo).toEqual({});
    expect(messageFor(channels[1], 'b', { id: 'g2', fields: { kenmerk: 'g1', gelukt: true } }).refersTo).toEqual({ opdracht: 'g1' });
  });

  it('delivers the order and the answer back, and stops when nothing follows', () => {
    const calls = [];
    const receive = (cell, article, refersTo, inputs) => {
      calls.push({ cell, article, refersTo, inputs: Object.keys(inputs) });
      if (cell === 'b') return [{ id: 'g2', establishes: 'voorwaarden#1', name: 'bijgeschreven', fields: { kenmerk: inputs.kenmerk.value, gelukt: true } }];
      return [{ id: 'g3', establishes: 'beleid#2', name: 'betaald', fields: {} }];
    };
    const recorded = deliver({ id: 'g1', name: 'opdracht', fields: { bedrag: 5 } }, 'a', channels, receive);
    expect(calls).toEqual([
      { cell: 'b', article: 'voorwaarden#1', refersTo: {}, inputs: ['kenmerk', 'bedrag'] },
      { cell: 'a', article: 'beleid#2', refersTo: { opdracht: 'g1' }, inputs: ['gelukt'] },
    ]);
    expect(recorded.map((r) => [r.cellId, r.gram.id])).toEqual([
      ['b', 'g2'],
      ['a', 'g3'],
    ]);
  });

  it('refuses a loop', () => {
    const loop = [{ from: { cell: 'a', event: 'x' }, to: { cell: 'a', article: 'w#1' }, inputs: {} }];
    expect(() => deliver({ id: '1', name: 'x' }, 'a', loop, () => [{ id: '2', name: 'x' }])).toThrow(/rond/);
  });
});
