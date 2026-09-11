import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import WorldTimeline from './WorldTimeline.vue';
import { worldFixture } from '../testing/worldFixture.js';
import { timelineMoments } from '../world/snapshot.js';

function mountTimeline(props = {}) {
  return mount(WorldTimeline, { props: { snapshot: worldFixture, ...props } });
}

describe('de tijdlijn onderaan', () => {
  it('zet de klok erbij, en zegt dat het logische tijd is', () => {
    const wrapper = mountTimeline();
    expect(wrapper.find('nldd-tag').attributes('text')).toBe('Klok: 01-02-2025');
    expect(wrapper.text()).toContain('logische tijd');
  });

  it('zet elk moment als punt op de lijn', () => {
    const wrapper = mountTimeline();
    const moments = timelineMoments(worldFixture);
    const points = wrapper.findAll('nldd-step-indicator-item');
    expect(points).toHaveLength(moments.length);
    expect(points.map((point) => point.attributes('status'))).toStrictEqual(moments.map((point) => point.step));
    expect(wrapper.find('nldd-step-indicator').attributes('current')).toBe(
      String(moments.findIndex((point) => point.isClock) + 1),
    );
  });

  it('noemt bij één gram de naam en bij meer het aantal', () => {
    const labels = mountTimeline()
      .findAll('nldd-step-indicator-item')
      .map((point) => point.attributes('text'));
    expect(labels).toContain('01-03-2024 · aanslag_vastgesteld');
    expect(labels.some((label) => label.endsWith('grammen'))).toBe(true);
    expect(labels).toContain('01-02-2025 · klok');
  });

  it('zet de klok apart op de lijn met een klok-icoon', () => {
    const clockPoint = mountTimeline()
      .findAll('nldd-step-indicator-item')
      .find((point) => point.attributes('status') === 'current');
    expect(clockPoint.attributes('icon')).toBe('clock');
  });

  it('spoelt vooruit tot de gekozen dag, nooit verder terug dan de klok', async () => {
    const wrapper = mountTimeline();
    const field = wrapper.find('nldd-date-field');
    expect(field.attributes('min')).toBe(worldFixture.clock);
    expect(field.attributes('value')).toBe(worldFixture.clock);

    field.element.dispatchEvent(new CustomEvent('change', { detail: { value: '2027-04-01' } }));
    await wrapper.vm.$nextTick();
    await wrapper.find('nldd-button').trigger('click');
    expect(wrapper.emitted('advance')).toStrictEqual([['2027-04-01']]);
  });

  it('spoelt niet naar een dag vóór de klok', async () => {
    const wrapper = mountTimeline();
    wrapper.find('nldd-date-field').element.dispatchEvent(
      new CustomEvent('change', { detail: { value: '2024-01-01' } }),
    );
    await wrapper.vm.$nextTick();
    expect(wrapper.find('nldd-button').attributes('disabled')).toBe('true');
  });
});
