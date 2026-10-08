import { describe, expect, it } from 'vitest';
import { ANSWERED, LOOP, channelKey, deliver, deliveryErrors, leaves, messageFor, redeliver } from './channels.js';

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
    const { recorded, undelivered } = deliver({ id: 'g1', name: 'opdracht', fields: { bedrag: 5 } }, 'a', channels, receive);
    expect(undelivered).toEqual([]);
    expect(calls).toEqual([
      { cell: 'b', article: 'voorwaarden#1', refersTo: {}, inputs: ['kenmerk', 'bedrag'] },
      { cell: 'a', article: 'beleid#2', refersTo: { opdracht: 'g1' }, inputs: ['gelukt'] },
    ]);
    expect(recorded.map((r) => [r.cellId, r.gram.id])).toEqual([
      ['b', 'g2'],
      ['a', 'g3'],
    ]);
  });

  it('refuses a loop, as a message that did not arrive', () => {
    const loop = [{ from: { cell: 'a', event: 'x' }, to: { cell: 'a', article: 'w#1' }, inputs: {} }];
    const { undelivered } = deliver({ id: '1', name: 'x' }, 'a', loop, () => [{ id: '2', name: 'x' }], { caseId: 'z' });
    expect(undelivered).toHaveLength(1);
    expect(undelivered[0]).toMatchObject({ cellId: 'a', gramId: '1', tag: { caseId: 'z' } });
    expect(undelivered[0].error).toMatch(/rond/);
    expect(undelivered[0].loop).toBeTruthy();
  });

  it('keeps a loop in the outbox, although the first receiver answers that it already has it', () => {
    const loop = [{ from: { cell: 'a', event: 'x' }, to: { cell: 'a', article: 'w#1' }, inputs: {} }];
    const first = deliver({ id: '1', name: 'x' }, 'a', loop, () => [{ id: '2', name: 'x' }], { caseId: 'z' });
    let calls = 0;
    const answered = () => {
      calls += 1;
      const e = new Error('al beantwoord');
      e.name = ANSWERED;
      throw e;
    };
    const again = redeliver(first.undelivered, loop, () => ({ id: '1', name: 'x' }), answered);
    expect(again.undelivered).toEqual(first.undelivered);
    expect(calls).toBe(0);
    expect(LOOP).toBe('loop');
  });

  it('offers a loop again once the channels changed, and drops it when it was answered', () => {
    const loop = [{ from: { cell: 'a', event: 'x' }, to: { cell: 'a', article: 'w#1' }, inputs: {} }];
    const first = deliver({ id: '1', name: 'x' }, 'a', loop, () => [{ id: '2', name: 'x' }], { caseId: 'z' });
    const fixed = [{ from: { cell: 'a', event: 'x' }, to: { cell: 'b', article: 'w#1' }, inputs: {} }];
    const answered = () => {
      const e = new Error('al beantwoord');
      e.name = ANSWERED;
      throw e;
    };
    expect(redeliver(first.undelivered, fixed, () => ({ id: '1', name: 'x' }), answered).undelivered).toEqual([]);
    const renamed = loop.map((c) => ({ ...c, inputs: { a: 'b' } }));
    expect(redeliver(first.undelivered, renamed, () => ({ id: '1', name: 'x' }), answered).undelivered).toEqual([]);
  });

  /** Een cel die faalt zolang `down` aan staat, en anders antwoordt zoals hierboven. */
  function network() {
    const net = { down: new Set(), calls: [], answered: new Set() };
    net.receive = (cell, article, refersTo, inputs, sentAt) => {
      net.calls.push({ cell, sentAt });
      if (net.down.has(cell)) throw new Error(`${cell} onbereikbaar`);
      if (cell === 'b') return [{ id: `b-${inputs.kenmerk.value}`, effective_at: sentAt ?? 'nu', establishes: 'voorwaarden#1', fields: { kenmerk: inputs.kenmerk.value, gelukt: true } }];
      if (net.answered.has(refersTo.opdracht)) {
        const e = new Error('al beantwoord');
        e.name = ANSWERED;
        throw e;
      }
      net.answered.add(refersTo.opdracht);
      return [{ id: `a-${refersTo.opdracht}`, establishes: 'beleid#2', name: 'betaald', fields: {} }];
    };
    return net;
  }
  const order = { id: 'g1', name: 'opdracht', effective_at: '2025-01-01T00:00:00+01:00', fields: { bedrag: 5 } };

  it('keeps a lost answer in the outbox, and sends each message at the moment of its gram', () => {
    const net = network();
    net.down.add('a');
    const { recorded, undelivered } = deliver(order, 'a', channels, net.receive, { caseId: 'z1' });
    expect(recorded.map((r) => r.gram.id)).toEqual(['b-g1']);
    expect(net.calls.map((c) => c.sentAt)).toEqual([order.effective_at, order.effective_at]);
    expect(undelivered).toEqual([{ cellId: 'b', gramId: 'b-g1', channel: channelKey(channels[1]), error: 'a onbereikbaar', tag: { caseId: 'z1' } }]);
  });

  it('delivers the outbox again, now, and keeps what still fails', () => {
    const net = network();
    net.down.add('b');
    const first = deliver(order, 'a', channels, net.receive, { caseId: 'z1' });
    expect(first.undelivered.map((u) => [u.cellId, u.gramId])).toEqual([['a', 'g1']]);
    const grams = { a: [order], b: [] };
    const gramOf = (cell, id) => grams[cell].find((g) => g.id === id);

    // Nog steeds onbereikbaar: het blijft in de outbox, met dezelfde tag.
    const still = redeliver(first.undelivered, channels, gramOf, net.receive);
    expect(still.recorded).toEqual([]);
    expect(still.undelivered).toEqual(first.undelivered);

    // Weer bereikbaar: de bank ontvangt het nu (zonder `sentAt`), en het antwoord gaat verder.
    net.down.clear();
    net.calls = [];
    const again = redeliver(first.undelivered, channels, gramOf, net.receive);
    expect(again.undelivered).toEqual([]);
    expect(again.recorded.map((r) => [r.cellId, r.gram.id])).toEqual([
      ['b', 'b-g1'],
      ['a', 'a-g1'],
    ]);
    expect(net.calls.map((c) => c.sentAt)).toEqual([null, 'nu']);
  });

  it('counts an answer that was already received as delivered', () => {
    const net = network();
    deliver(order, 'a', channels, net.receive);
    const { recorded, undelivered } = redeliver(
      [{ cellId: 'b', gramId: 'b-g1', channel: channelKey(channels[1]), error: 'x', tag: {} }],
      channels,
      () => ({ id: 'b-g1', establishes: 'voorwaarden#1', fields: { kenmerk: 'g1', gelukt: true } }),
      net.receive,
    );
    expect(recorded).toEqual([]);
    expect(undelivered).toEqual([]);
  });

  it('keeps an entry whose channel or gram is gone, saying why', () => {
    const entry = { cellId: 'a', gramId: 'weg', channel: channelKey(channels[0]), error: 'x', tag: {} };
    const { undelivered } = redeliver([entry], channels, () => undefined, () => []);
    expect(undelivered[0].error).toMatch(/weg/);
  });

  it('goes on with the next entry when one throws, and keeps only that one', () => {
    const net = network();
    const second = { id: 'g2', name: 'opdracht', effective_at: order.effective_at, fields: { bedrag: 7 } };
    const grams = { a: [order, second], b: [] };
    const outbox = [order, second].map((g) => ({ cellId: 'a', gramId: g.id, channel: channelKey(channels[0]), error: 'x', tag: { caseId: g.id } }));
    const gramOf = (cell, id) => {
      if (id === 'g1') throw new Error('kapot');
      return grams[cell].find((g) => g.id === id);
    };
    const { recorded, undelivered } = redeliver(outbox, channels, gramOf, net.receive);
    expect(undelivered).toEqual([{ ...outbox[0], error: 'kapot' }]);
    expect(recorded.map((r) => r.gram.id)).toEqual(['b-g2', 'a-g2']);
  });

  it('gives per case the error of what is still in the outbox', () => {
    const outbox = [
      { error: 'eerste', tag: { caseId: 'z1' } },
      { error: 'tweede', tag: { caseId: 'z1' } },
      { error: 'andere', tag: { caseId: 'z2' } },
      { error: 'zonder zaak', tag: {} },
    ];
    expect(deliveryErrors(outbox)).toEqual({ z1: 'eerste; tweede', z2: 'andere' });
    expect(deliveryErrors([])).toEqual({});
    expect(deliveryErrors(undefined)).toEqual({});
  });
});
