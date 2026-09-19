import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import MachineEmptyState from './MachineEmptyState.vue';

// Het knoplabel draagt twee dingen tegelijk die alleen in de tekst zichtbaar
// zijn en die een volgende bewerking dus stilzwijgend kan platslaan:
//
//  1. de scope - de aanvraag kent alleen een wet en levert een voorstel per
//     gewijzigd artikel op, dus het label mag geen artikel-scope suggereren;
//  2. de vorm - gebiedende wijs waar de klik de actie hier uitvoert, heel
//     werkwoord waar hij je eerst ergens heen brengt (login of traject).
//
// Geen enkele andere test raakt deze strings, terwijl de knop in vier panes
// staat (Machine en YAML, in de editor en in de Bibliotheek).

function mountEmpty(needs = '') {
  return mount(MachineEmptyState, {
    props: { canEnrich: true, needs },
  });
}

function enrichLabel(wrapper) {
  return wrapper.find('[data-testid="enrich-btn"]').attributes('text');
}

describe('MachineEmptyState', () => {
  it('noemt de wet en niet het artikel waar de knop de actie hier uitvoert', () => {
    const label = enrichLabel(mountEmpty(''));
    expect(label).toBe('Verrijk deze wet');
    expect(label).not.toMatch(/artikel/i);
  });

  it.each(['login', 'traject'])('gebruikt het hele werkwoord waar de knop routeert (%s)', (needs) => {
    const label = enrichLabel(mountEmpty(needs));
    expect(label).toBe('Deze wet verrijken');
    expect(label).not.toMatch(/artikel/i);
  });

  it('zegt in de ondersteunende tekst dat de verrijking de hele wet raakt', () => {
    const supporting = mountEmpty('')
      .find('[data-testid="no-machine-readable"]')
      .attributes('supporting-text');
    expect(supporting).toContain('de hele wet');
  });
});
