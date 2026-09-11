import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import ObservationLog from './ObservationLog.vue';
import { cloneWorld, worldFixture } from '../testing/worldFixture.js';

function mountLog(snapshot = worldFixture) {
  return mount(ObservationLog, { props: { snapshot } });
}

describe('het observatielog', () => {
  it('staat gelabeld als meetinstrument dat in een echte deployment niet bestaat', () => {
    const banner = mountLog().find('nldd-banner');
    expect(banner.attributes('text')).toBe('Meetinstrument van de testopstelling');
    expect(banner.attributes('supporting-text')).toContain('bestaat in een echte deployment niet');
    expect(banner.attributes('supporting-text')).toContain('Geen enkele cel kan dit overzicht opvragen');
  });

  it('zet elk contact over een celgrens als rij, met vraag, moment en antwoord', () => {
    const wrapper = mountLog();
    // Eén kopregel plus een rij per contact.
    expect(wrapper.findAll('nldd-table-row')).toHaveLength(worldFixture.crossings.length + 1);

    const crossing = worldFixture.crossings[0];
    const texts = wrapper.findAll('nldd-text-cell').map((cell) => cell.attributes('text'));
    expect(texts).toContain(crossing.answer.cell);
    expect(texts).toContain(crossing.asked_by);

    const supporting = wrapper
      .findAll('nldd-text-cell')
      .map((cell) => cell.attributes('supporting-text'))
      .filter(Boolean);
    expect(supporting).toContain(`lexostatus: ${crossing.answer.name}`);
    expect(supporting.some((text) => text.includes(crossing.signature))).toBe(true);
    expect(supporting.some((text) => text.includes('bsn:'))).toBe(true);
  });

  it('geeft een vastgesteld antwoord met zijn waarden', () => {
    const texts = mountLog()
      .findAll('nldd-text-cell')
      .map((cell) => cell.attributes('text'));
    expect(texts.some((text) => text?.includes('toetsingsinkomen: 81000'))).toBe(true);
  });

  it('geeft "niets vastgesteld" als antwoord en niet als fout', () => {
    const snapshot = cloneWorld();
    snapshot.crossings = [
      {
        asked_by: 'cel:toeslagen',
        signature: 'GESIMULEERDE-ONDERTEKENING door cel:toeslagen',
        params: { bsn: '999993653' },
        answer: {
          cell: 'belastingdienst',
          name: 'toetsingsinkomen',
          op_moment: '2023-01-01',
          outcome: { not_established: { reason: 'niets vastgesteld op dit moment' } },
        },
      },
    ];
    const wrapper = mountLog(snapshot);
    const cells = wrapper.findAll('nldd-text-cell');
    const answer = cells.find((cell) => cell.attributes('text') === 'niets vastgesteld');
    expect(answer).toBeDefined();
    expect(answer.attributes('color')).toBe('secondary');
    expect(wrapper.findAll('nldd-banner')).toHaveLength(1);
  });

  it('blijft leeg zolang geen cel iets bij een ander opvraagt', () => {
    const snapshot = cloneWorld();
    snapshot.crossings = [];
    const wrapper = mountLog(snapshot);
    expect(wrapper.findAll('nldd-table-row')).toHaveLength(1);
    expect(wrapper.find('nldd-table').attributes('empty-text')).toContain('Nog geen contact');
  });
});
