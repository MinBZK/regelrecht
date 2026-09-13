import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import ActionPanel from './ActionPanel.vue';
import { cloneWorld, worldFixture } from '../testing/worldFixture.js';

/** Een waarde in een veld zetten zoals een ontwerpsysteem-veld dat meldt. */
async function fill(wrapper, element, value) {
  element.element.dispatchEvent(new CustomEvent('input', { detail: { value } }));
  await wrapper.vm.$nextTick();
}

function mountPanel(snapshot = worldFixture, props = {}) {
  return mount(ActionPanel, { props: { snapshot, ...props } });
}

describe('het actiepaneel', () => {
  it('groepeert de acties per actor', () => {
    const wrapper = mountPanel();
    const actors = [...new Set(worldFixture.actions.map((action) => action.actor))];
    for (const actor of actors) expect(wrapper.text()).toContain(actor);
    expect(wrapper.findAll('nldd-card')).toHaveLength(worldFixture.actions.length);
  });

  it('zet het formulier uit de actie, met het veld van het juiste type', () => {
    const wrapper = mountPanel();
    const card = wrapper.findAll('nldd-card')[0];
    const action = worldFixture.actions[0];
    expect(action.form.map((field) => field.type)).toStrictEqual(['string', 'number', 'date']);
    expect(card.findAll('nldd-text-field')).toHaveLength(1);
    expect(card.findAll('nldd-number-field')).toHaveLength(1);
    expect(card.findAll('nldd-date-field')).toHaveLength(1);
    const labels = card.findAll('nldd-form-field').map((field) => field.attributes('label'));
    expect(labels).toStrictEqual(['Bsn', 'Jaar', 'Ondertekend op']);
    const types = card.findAll('nldd-form-field').map((field) => field.attributes('supporting-label'));
    expect(types).toStrictEqual(['string', 'number', 'date · dd-mm-jjjj']);
  });

  // Het hele punt van `type: date`: een datum krijgt het datumveld en geen
  // tekstveld. Dat veld toont dd-mm-jjjj en geeft ISO terug, dus de app hoeft
  // zelf niets om te rekenen — en de invuller hoeft niet te raden.
  it('stuurt een datum als ISO door, precies zoals het datumveld hem geeft', async () => {
    const wrapper = mountPanel();
    const card = wrapper.findAll('nldd-card')[0];
    await fill(wrapper, card.find('nldd-text-field'), '999993653');
    await fill(wrapper, card.find('nldd-number-field'), 2025);
    await fill(wrapper, card.find('nldd-date-field'), '2024-01-09');
    await card.find('form').trigger('submit');

    const [{ values }] = wrapper.emitted('run')[0];
    expect(values.ondertekend_op).toBe('2024-01-09');
  });

  it('toont de weigering van de server bij de actie die hem uitlokte', () => {
    const melding = "parameter 'ondertekend_op' is geen datum: '09-01-2024' (verwacht jjjj-mm-dd)";
    const wrapper = mountPanel(worldFixture, {
      error: { action: worldFixture.actions[0].id, message: melding },
    });
    const cards = wrapper.findAll('nldd-card');
    const kritiek = cards[0].findAll('nldd-banner').filter((banner) => banner.attributes('variant') === 'critical');
    expect(kritiek).toHaveLength(1);
    expect(kritiek[0].attributes('supporting-text')).toBe(melding);

    // En nergens anders: een andere actie heeft hier niets mee te maken.
    for (const card of cards.slice(1)) {
      expect(card.findAll('nldd-banner').filter((b) => b.attributes('variant') === 'critical')).toHaveLength(0);
    }
  });

  it('zegt of een actie nu kan, en waarom niet', () => {
    const snapshot = cloneWorld();
    snapshot.actions[1].available = false;
    snapshot.actions[1].unavailable_reason = "cel 'toeslagen' heeft nog geen aanvraag in kroniek 'aanvragen'";
    const wrapper = mountPanel(snapshot);
    const tags = wrapper.findAll('nldd-tag').map((tag) => tag.attributes('text'));
    expect(tags).toContain('kan nu');
    expect(tags).toContain('kan nu niet');
    const banner = wrapper.find('nldd-banner');
    expect(banner.attributes('variant')).toBe('warning');
    expect(banner.attributes('supporting-text')).toContain('nog geen aanvraag');
  });

  it('voert de actie uit met de ingevulde waarden, elk in zijn eigen type', async () => {
    const wrapper = mountPanel();
    const card = wrapper.findAll('nldd-card')[0];
    await fill(wrapper, card.find('nldd-text-field'), '999993653');
    await fill(wrapper, card.find('nldd-number-field'), 2025);
    await fill(wrapper, card.find('nldd-date-field'), '2024-01-09');
    await card.find('form').trigger('submit');

    expect(wrapper.emitted('run')).toHaveLength(1);
    const [{ action, values }] = wrapper.emitted('run')[0];
    expect(action.id).toBe(worldFixture.actions[0].id);
    expect(values).toStrictEqual({ bsn: '999993653', jaar: 2025, ondertekend_op: '2024-01-09' });
  });

  // Wat de wereld al weet, staat er al in — en het veld zegt erbij dat het een
  // voorstel is. Zodra iemand er iets anders van maakt, is het zijn opgave en
  // staat die melding er niet meer.
  it('begint met de voorinvulling uit het beeld, en markeert haar als zodanig', async () => {
    const wrapper = mountPanel();
    const card = wrapper.findAll('nldd-card')[0];
    const { prefill } = worldFixture.actions[0];
    expect(card.find('nldd-text-field').attributes('value')).toBe(prefill.bsn);
    expect(card.find('nldd-number-field').attributes('value')).toBe(String(prefill.jaar));
    expect(card.find('nldd-date-field').attributes('value')).toBe(prefill.ondertekend_op);
    expect(card.findAll('nldd-form-field-help-text')).toHaveLength(3);

    await card.find('form').trigger('submit');
    const [{ values }] = wrapper.emitted('run')[0];
    expect(values).toStrictEqual(prefill);

    await fill(wrapper, card.find('nldd-text-field'), '999993756');
    expect(card.findAll('nldd-form-field-help-text')).toHaveLength(2);
  });

  it('vraagt eerst om een leeg veld in plaats van het als niets te versturen', async () => {
    // Zonder voorinvulling, want dit gaat over het veld dat leeg blijft: met een
    // voorstel erin is er niets om over te struikelen.
    const snapshot = cloneWorld();
    for (const action of snapshot.actions) action.prefill = {};
    const wrapper = mountPanel(snapshot);
    const card = wrapper.findAll('nldd-card')[0];
    await card.find('form').trigger('submit');

    expect(wrapper.emitted('run')).toBeUndefined();
    expect(card.findAll('nldd-form-field-error-text')).toHaveLength(3);
    expect(card.find('nldd-text-field').attributes('invalid')).toBe('true');
    expect(card.find('nldd-date-field').attributes('invalid')).toBe('true');
    expect(card.find('nldd-text-field').attributes('error-message')).toBe(
      card.find('nldd-form-field-error-text').attributes('id'),
    );

    // Zodra er iets staat, is de melding weg.
    await fill(wrapper, card.find('nldd-text-field'), '999993653');
    expect(card.findAll('nldd-form-field-error-text')).toHaveLength(2);
    await fill(wrapper, card.find('nldd-date-field'), '2024-01-09');
    expect(card.findAll('nldd-form-field-error-text')).toHaveLength(1);
  });

  // De kaart van een besluit dat er al ligt. In de fixture besloot cel
  // 'toeslagen' op 01-04-2024 over deze zaak; wie hetzelfde besluit opnieuw
  // aanroept legt een tweede decretogram, en dat hoort niet per ongeluk te
  // gebeuren.
  describe('een besluit waarover al besloten is', () => {
    /** De kaart van een `decides`-actie, met de zaak ingevuld. */
    async function decisionCard(bsn) {
      const wrapper = mountPanel();
      const index = worldFixture.actions.findIndex((action) => action.effect.soort === 'decides');
      const card = wrapper.findAll('nldd-card')[index];
      await fill(wrapper, card.find('nldd-text-field'), bsn);
      return { wrapper, card };
    }

    it('zegt "al besloten op" in plaats van "kan nu"', async () => {
      const { card } = await decisionCard('999993653');
      expect(card.find('nldd-tag').attributes('text')).toBe('al besloten op 01-04-2024');
    });

    // "Al besloten op …" komt in de plaats van "kan nu", nooit in de plaats van
    // "kan nu niet": een actie die de wereld nú weigert, hoort dat te blijven
    // zeggen. Anders staat er een tag die zegt dat er iets ligt boven een knop
    // die niets kan.
    it('laat "kan nu niet" staan naast wat er al ligt', async () => {
      const snapshot = cloneWorld();
      const index = snapshot.actions.findIndex((action) => action.effect.soort === 'decides');
      snapshot.actions[index].available = false;
      snapshot.actions[index].unavailable_reason = 'de aanvraag ligt er nog niet';
      const wrapper = mountPanel(snapshot);
      const card = wrapper.findAll('nldd-card')[index];
      await fill(wrapper, card.find('nldd-text-field'), '999993653');

      expect(card.findAll('nldd-tag').map((tag) => tag.attributes('text'))).toStrictEqual([
        'al besloten op 01-04-2024',
        'kan nu niet',
      ]);
    });

    it('laat "kan nu" staan voor een zaak waarover nog niets ligt', async () => {
      const { card } = await decisionCard('999999999');
      expect(card.find('nldd-tag').attributes('text')).toBe('kan nu');
    });

    it('vraagt eerst om bevestiging en voert daarna wél uit', async () => {
      const { wrapper, card } = await decisionCard('999993653');
      await card.find('form').trigger('submit');

      // Nog niets uitgevoerd: er staat een vraag.
      expect(wrapper.emitted('run')).toBeUndefined();
      const dialog = card.find('nldd-inline-dialog');
      expect(dialog.attributes('text')).toBe('Er ligt al een besluit');
      expect(dialog.attributes('supporting-text')).toContain('zorgtoeslag/999993653');
      expect(dialog.attributes('supporting-text')).toContain('01-04-2024');

      // Een tweede besluit is legitiem, dus de weg ernaartoe blijft open.
      const confirm = dialog.findAll('nldd-button').find((button) => button.attributes('text') === 'Toch besluiten');
      await confirm.trigger('click');
      expect(wrapper.emitted('run')).toHaveLength(1);
      expect(wrapper.emitted('run')[0][0].values).toStrictEqual({ bsn: '999993653' });
    });

    it('laat de bevestiging weer los zonder iets te doen', async () => {
      const { wrapper, card } = await decisionCard('999993653');
      await card.find('form').trigger('submit');
      const annuleren = card
        .findAll('nldd-button')
        .find((button) => button.attributes('text') === 'Annuleren');
      await annuleren.trigger('click');

      expect(wrapper.emitted('run')).toBeUndefined();
      expect(card.find('nldd-inline-dialog').exists()).toBe(false);
    });
  });

  it('zegt het als het wereldbestand geen acties beschrijft', () => {
    const snapshot = cloneWorld();
    snapshot.actions = [];
    const wrapper = mountPanel(snapshot);
    expect(wrapper.find('nldd-inline-dialog').attributes('text')).toBe('Geen acties');
  });
});
