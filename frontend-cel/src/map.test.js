import { describe, expect, it } from 'vitest';
import { collapse, columnLayout, edgeKey, followEvent, litShown, nodeText } from './map.js';

const article = (regulation, label) => ({
  id: `article:${regulation}#${label}`,
  kind: 'article',
  label,
  regulation,
  source: { law: `${regulation}#${label}` },
});

// A map as the runtime gives it: the portal event, established and extended
// by Wpp 102, hooked by Awb 4:2 and 4:13, Wpp 107 decides on 102 and reads
// Awb 4:2 as a source.
const map = {
  process: 'p',
  nodes: [
    { id: 'process:p', kind: 'process', label: 'p', source: { config: 'process', anchor: 'id' } },
    { id: 'stream:a', kind: 'stream', label: 'a', source: { config: 'stream/a', anchor: 'events' } },
    { id: 'event:a/ontvangen', kind: 'event', label: 'ontvangen', source: { config: 'stream/a', anchor: 'ontvangen' } },
    { id: 'event:a/besluit', kind: 'event', label: 'besluit', source: { config: 'stream/a', anchor: 'besluit' } },
    article('wpp', '102'),
    article('wpp', '107'),
    article('awb', '4:2'),
    article('awb', '4:13'),
  ],
  edges: [
    { from: 'process:p', to: 'event:a/ontvangen', kind: 'portal' },
    { from: 'stream:a', to: 'event:a/ontvangen', kind: 'records' },
    { from: 'stream:a', to: 'event:a/besluit', kind: 'records' },
    { from: 'event:a/ontvangen', to: 'article:wpp#102', kind: 'establishes' },
    { from: 'event:a/ontvangen', to: 'article:wpp#102', kind: 'extends' },
    { from: 'event:a/ontvangen', to: 'article:awb#4:2', kind: 'hook' },
    { from: 'event:a/ontvangen', to: 'article:awb#4:13', kind: 'hook' },
    { from: 'article:wpp#107', to: 'article:wpp#102', kind: 'decides_on' },
    { from: 'article:wpp#107', to: 'article:awb#4:2', kind: 'source' },
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

  it('keeps an edge between two folded laws, from law to law', () => {
    const m = collapse(map, new Set());
    expect(m.edges).toContainEqual({ from: 'law:wpp', to: 'law:awb', kind: 'source' });
  });

  it('keeps parallel edges of different kinds apart', () => {
    const m = collapse(map, new Set());
    const toWpp = m.edges.filter((e) => e.from === 'event:a/ontvangen' && e.to === 'law:wpp');
    expect(toWpp.map((e) => e.kind).sort()).toEqual(['establishes', 'extends']);
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

  it('follows a source one step outward, and a shared source does not bridge', () => {
    // 102 and 110 both read the Kieswet; 110 is established by another
    // event, and Kieswet G 4 reads further from G 2.
    const shared = {
      process: 'p',
      nodes: [
        { id: 'event:a/ontvangen', kind: 'event', label: 'ontvangen' },
        { id: 'event:a/fusie', kind: 'event', label: 'fusie' },
        article('wpp', '102'),
        article('wpp', '110'),
        article('kieswet', 'G 4'),
        article('kieswet', 'G 2'),
      ],
      edges: [
        { from: 'event:a/ontvangen', to: 'article:wpp#102', kind: 'establishes' },
        { from: 'event:a/fusie', to: 'article:wpp#110', kind: 'establishes' },
        { from: 'article:wpp#102', to: 'article:kieswet#G 4', kind: 'source' },
        { from: 'article:wpp#110', to: 'article:kieswet#G 4', kind: 'source' },
        { from: 'article:kieswet#G 4', to: 'article:kieswet#G 2', kind: 'prefill' },
      ],
    };
    const lit = followEvent(shared, 'event:a/ontvangen');
    expect([...lit.nodes].sort()).toEqual(['article:kieswet#G 4', 'article:wpp#102', 'event:a/ontvangen'].sort());
    expect([...lit.edges].sort()).toEqual([0, 2]);
  });

  it('follows decides_on transitively per direction, never back and forth', () => {
    // 107 decides on 102 and on 110; 120 decides on 107. From the event
    // (102): 107 and 120 decide on it, but 110 is another subject of 107.
    const chain = {
      process: 'p',
      nodes: [
        { id: 'event:a/ontvangen', kind: 'event', label: 'ontvangen' },
        article('wpp', '102'),
        article('wpp', '107'),
        article('wpp', '110'),
        article('wpp', '120'),
      ],
      edges: [
        { from: 'event:a/ontvangen', to: 'article:wpp#102', kind: 'establishes' },
        { from: 'article:wpp#107', to: 'article:wpp#102', kind: 'decides_on' },
        { from: 'article:wpp#107', to: 'article:wpp#110', kind: 'decides_on' },
        { from: 'article:wpp#120', to: 'article:wpp#107', kind: 'decides_on' },
      ],
    };
    const lit = followEvent(chain, 'event:a/ontvangen');
    expect(lit.nodes.has('article:wpp#107')).toBe(true);
    expect(lit.nodes.has('article:wpp#120')).toBe(true);
    expect(lit.nodes.has('article:wpp#110')).toBe(false);
  });

  it('without an event there is no filter', () => {
    expect(followEvent(map, null)).toBeNull();
  });
});

describe('litShown', () => {
  const lit = followEvent(map, 'event:a/ontvangen');

  it('shows a lit article in a folded law as the law', () => {
    const shown = litShown(map, lit, new Set());
    expect(shown.nodes.has('law:wpp')).toBe(true);
    expect(shown.nodes.has('article:wpp#102')).toBe(false);
    expect(litShown(map, lit, new Set(['wpp'])).nodes.has('article:wpp#102')).toBe(true);
  });

  it('drops a lit edge that folds into one law', () => {
    const shown = litShown(map, lit, new Set());
    expect(shown.edges.has(edgeKey('law:wpp', 'law:wpp', 'decides_on'))).toBe(false);
    expect(litShown(map, lit, new Set(['wpp'])).edges.has(edgeKey('article:wpp#107', 'article:wpp#102', 'decides_on'))).toBe(true);
  });

  it('merges lit edges that fold together, as collapse does', () => {
    const shown = litShown(map, lit, new Set());
    const hooks = [...shown.edges].filter((k) => k.endsWith('|hook'));
    expect(hooks).toEqual([edgeKey('event:a/ontvangen', 'law:awb', 'hook')]);
    const keys = new Set(collapse(map, new Set()).edges.map((e) => edgeKey(e.from, e.to, e.kind)));
    for (const k of shown.edges) expect(keys.has(k)).toBe(true);
  });

  it('is null without a filter', () => {
    expect(litShown(map, null, new Set())).toBeNull();
  });
});

describe('columnLayout', () => {
  it('puts configuration left and the law right, stable within a column', () => {
    const pos = columnLayout(collapse(map, new Set()).nodes);
    expect(pos.get('process:p').x).toBeLessThan(pos.get('event:a/ontvangen').x);
    expect(pos.get('event:a/ontvangen').x).toBeLessThan(pos.get('law:wpp').x);
    expect(pos.get('law:awb').y).toBeLessThan(pos.get('law:wpp').y);
  });

  it('sorts articles in their numeric order', () => {
    const pos = columnLayout([article('awb', '4:13'), article('awb', '4:2'), article('wpp', '9'), article('wpp', '102')]);
    expect(pos.get('article:awb#4:2').y).toBeLessThan(pos.get('article:awb#4:13').y);
    expect(pos.get('article:wpp#9').y).toBeLessThan(pos.get('article:wpp#102').y);
  });
});

describe('nodeText', () => {
  it('names the kind, the law of an article and the count of a folded law', () => {
    const m = collapse(map, new Set(['awb']));
    expect(nodeText(m.nodes.find((n) => n.id === 'stream:a'))).toBe('stroom: a');
    expect(nodeText(m.nodes.find((n) => n.id === 'article:awb#4:2'))).toBe('awb art. 4:2');
    expect(nodeText(m.nodes.find((n) => n.id === 'law:wpp'))).toBe('wet wpp (2)');
  });
});
