import { describe, expect, it } from 'vitest';
import { collapse, columnLayout, followEvent } from './map.js';

// A map as the runtime gives it: the portal event, established by Wpp 102,
// hooked by Awb 4:2 and 4:13, Wpp 107 decides on 102.
const map = {
  process: 'p',
  nodes: [
    { id: 'process:p', kind: 'process', label: 'p', source: { config: 'process', anchor: 'id' } },
    { id: 'stream:a', kind: 'stream', label: 'a', source: { config: 'stream/a', anchor: 'events' } },
    { id: 'event:a/ontvangen', kind: 'event', label: 'ontvangen', source: { config: 'stream/a', anchor: 'ontvangen' } },
    { id: 'event:a/besluit', kind: 'event', label: 'besluit', source: { config: 'stream/a', anchor: 'besluit' } },
    { id: 'article:wpp#102', kind: 'article', label: '102', regulation: 'wpp', source: { law: 'wpp#102' } },
    { id: 'article:wpp#107', kind: 'article', label: '107', regulation: 'wpp', source: { law: 'wpp#107' } },
    { id: 'article:awb#4:2', kind: 'article', label: '4:2', regulation: 'awb', source: { law: 'awb#4:2' } },
    { id: 'article:awb#4:13', kind: 'article', label: '4:13', regulation: 'awb', source: { law: 'awb#4:13' } },
  ],
  edges: [
    { from: 'process:p', to: 'event:a/ontvangen', kind: 'portal' },
    { from: 'stream:a', to: 'event:a/ontvangen', kind: 'records' },
    { from: 'stream:a', to: 'event:a/besluit', kind: 'records' },
    { from: 'event:a/ontvangen', to: 'article:wpp#102', kind: 'establishes' },
    { from: 'event:a/ontvangen', to: 'article:awb#4:2', kind: 'hook' },
    { from: 'event:a/ontvangen', to: 'article:awb#4:13', kind: 'hook' },
    { from: 'article:wpp#107', to: 'article:wpp#102', kind: 'decides_on' },
    { from: 'event:a/besluit', to: 'article:wpp#107', kind: 'establishes' },
  ],
};

describe('collapse', () => {
  it('folds the articles of a law into one node with a count', () => {
    const m = collapse(map, new Set());
    const wpp = m.nodes.find((n) => n.id === 'law:wpp');
    expect(wpp).toMatchObject({ kind: 'law', label: 'wpp', count: 2 });
    expect(m.nodes.some((n) => n.kind === 'article')).toBe(false);
  });

  it('redirects edges to the law and drops edges inside one law', () => {
    const m = collapse(map, new Set());
    expect(m.edges).toContainEqual({ from: 'event:a/ontvangen', to: 'law:awb', kind: 'hook' });
    expect(m.edges.filter((e) => e.from === 'event:a/ontvangen' && e.to === 'law:awb')).toHaveLength(1);
    expect(m.edges.some((e) => e.from === 'law:wpp' && e.to === 'law:wpp')).toBe(false);
  });

  it('keeps an opened law as articles', () => {
    const m = collapse(map, new Set(['wpp']));
    expect(m.nodes.some((n) => n.id === 'article:wpp#102')).toBe(true);
    expect(m.nodes.some((n) => n.id === 'law:awb')).toBe(true);
    expect(m.nodes.some((n) => n.id === 'law:wpp')).toBe(false);
    expect(m.edges).toContainEqual({ from: 'article:wpp#107', to: 'article:wpp#102', kind: 'decides_on' });
  });
});

describe('followEvent', () => {
  it('lights up the chain of one event and nothing of another', () => {
    const lit = followEvent(map, 'event:a/ontvangen');
    expect([...lit.nodes].sort()).toEqual(
      ['article:awb#4:13', 'article:awb#4:2', 'article:wpp#102', 'article:wpp#107', 'event:a/ontvangen', 'process:p', 'stream:a'].sort(),
    );
    expect(lit.nodes.has('event:a/besluit')).toBe(false);
    // 107 is lit through its decision on 102, not through the other event.
    expect(lit.edges.has(map.edges.findIndex((e) => e.kind === 'decides_on'))).toBe(true);
    expect(lit.edges.has(map.edges.findIndex((e) => e.from === 'event:a/besluit' && e.kind === 'establishes'))).toBe(false);
  });

  it('without an event lights up everything', () => {
    expect(followEvent(map, null)).toBeNull();
  });
});

describe('columnLayout', () => {
  it('puts configuration left and the law right, stable within a column', () => {
    const pos = columnLayout(collapse(map, new Set()).nodes);
    expect(pos.get('process:p').x).toBeLessThan(pos.get('event:a/ontvangen').x);
    expect(pos.get('event:a/ontvangen').x).toBeLessThan(pos.get('law:wpp').x);
    expect(pos.get('law:awb').y).toBeLessThan(pos.get('law:wpp').y);
  });
});
