import { describe, expect, it, vi } from 'vitest';
import { cloneWorld, worldFixture } from '../testing/worldFixture.js';
import { createWorld } from './useWorld.js';

/** Een wereld-API die geeft wat de test wil, zonder verkeer. */
function fakeApi(overrides = {}) {
  return {
    fetchWorld: vi.fn(async () => worldFixture),
    runAction: vi.fn(async () => worldFixture),
    advanceTo: vi.fn(async () => worldFixture),
    updateSettings: vi.fn(async () => worldFixture),
    resetWorld: vi.fn(async () => worldFixture),
    askLexostatus: vi.fn(async () => ({ cell: 'belastingdienst', name: 'x', outcome: {} })),
    ...overrides,
  };
}

/** Hetzelfde beeld met één gram erbij, zoals een actie het zou opleveren. */
function worldWithExtraGram(moment = '2025-02-02') {
  const next = cloneWorld();
  next.cells
    .find((cell) => cell.id === 'burger')
    .chronicles[0].grams.push({
      kind: 'executogram',
      name: 'aanvraag_ingediend',
      intake: 'aanvraag',
      recording_actor: 'burger',
      grondslag: '',
      op_moment: moment,
      fields: {},
    });
  return next;
}

describe('de wereld in de browser', () => {
  it('haalt het beeld op en markeert daarbij niets als nieuw', async () => {
    const api = fakeApi();
    const world = createWorld(api);
    await world.load();
    expect(world.ready.value).toBe(true);
    expect(world.clock.value).toBe(worldFixture.clock);
    expect(world.previousCounts.value).toBeNull();
    expect(world.error.value).toBeNull();
  });

  it('neemt na een actie het nieuwe beeld over en verslaat wat erbij kwam', async () => {
    const next = worldWithExtraGram();
    const api = fakeApi({ runAction: vi.fn(async () => next) });
    const world = createWorld(api);
    await world.load();
    await world.act({ id: 'burger.aanvraag', label: 'Aanvraag indienen' }, { bsn: '1' });

    expect(api.runAction).toHaveBeenCalledWith('burger.aanvraag', { bsn: '1' });
    expect(world.snapshot.value).toBe(next);
    expect(world.result.value.label).toBe('Aanvraag indienen');
    expect(world.result.value.grams).toStrictEqual([
      { cell: 'burger', stream: 'aanvragen', kind: 'executogram', name: 'aanvraag_ingediend', opMoment: '2025-02-02' },
    ]);
    expect(world.previousCounts.value.get('burger|aanvragen')).toBe(
      worldFixture.cells.find((cell) => cell.id === 'burger').chronicles[0].grams.length,
    );
  });

  it('haalt het beeld apart op als het antwoord er geen draagt', async () => {
    const api = fakeApi({ runAction: vi.fn(async () => ({ events: ['iets gebeurde'] })) });
    const world = createWorld(api);
    await world.load();
    await world.act({ id: 'x', label: 'Iets' }, {});
    expect(api.fetchWorld).toHaveBeenCalledTimes(2);
    expect(world.snapshot.value).toBe(worldFixture);
  });

  it('spoelt vooruit en zegt tot wanneer', async () => {
    const api = fakeApi();
    const world = createWorld(api);
    await world.load();
    await world.advance('2027-04-01');
    expect(api.advanceTo).toHaveBeenCalledWith('2027-04-01');
    expect(world.result.value.label).toContain('2027-04-01');
  });

  it('laat na terugzetten niets als nieuw staan', async () => {
    const api = fakeApi();
    const world = createWorld(api);
    await world.load();
    await world.act({ id: 'x', label: 'Iets' }, {});
    await world.reset();
    expect(api.resetWorld).toHaveBeenCalled();
    expect(world.previousCounts.value).toBeNull();
    expect(world.result.value.grams).toStrictEqual([]);
  });

  it('geeft de fout van de server door en laat het beeld staan', async () => {
    const api = fakeApi({
      runAction: vi.fn(async () => {
        throw new Error("cel 'toeslagen' kan dit besluit nu niet nemen");
      }),
    });
    const world = createWorld(api);
    await world.load();
    await world.act({ id: 'x', label: 'Iets' }, {});
    expect(world.error.value).toContain("cel 'toeslagen'");
    expect(world.snapshot.value).toBe(worldFixture);
    expect(world.busy.value).toBe(false);

    world.dismissError();
    expect(world.error.value).toBeNull();
  });

  it('vraagt een lexostatus zonder het beeld te veranderen', async () => {
    const api = fakeApi();
    const world = createWorld(api);
    await world.load();
    const answer = await world.askLexostatus('belastingdienst', 'toetsingsinkomen', { bsn: '1' });
    expect(api.askLexostatus).toHaveBeenCalledWith('belastingdienst', 'toetsingsinkomen', { bsn: '1' });
    expect(answer.cell).toBe('belastingdienst');
    expect(world.result.value).toBeNull();
  });
});
