import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ConversionStatus from './ConversionStatus.vue';

// DS elements compile to raw custom elements under happy-dom; assert against
// attributes rather than shadow DOM.
function mountStatus(jobs) {
  return mount(ConversionStatus, { props: { jobs } });
}

describe('ConversionStatus', () => {
  it('renders nothing when there are no jobs', () => {
    const wrapper = mountStatus([]);
    expect(wrapper.find('.conversion-status').exists()).toBe(false);
  });

  it('shows a spinner for a running conversion (de-slugged title, no .md)', () => {
    const wrapper = mountStatus([{ id: '1', target_path: 'mijn-report.md', status: 'processing' }]);
    const indicator = wrapper.find('nldd-activity-indicator');
    expect(indicator.exists()).toBe(true);
    expect(indicator.attributes('text')).toContain('Mijn report');
    expect(indicator.attributes('text')).not.toContain('.md');
  });

  it('shows a spinner per running job, no error dialog (flag ON: failures are tasks)', () => {
    const wrapper = mountStatus([
      { id: '1', target_path: 'report.md', status: 'processing' },
      { id: '2', target_path: 'brief.md', status: 'processing' },
    ]);
    expect(wrapper.findAll('nldd-activity-indicator')).toHaveLength(2);
    expect(wrapper.find('nldd-inline-dialog').exists()).toBe(false);
  });

  // Flag OFF: the jobs endpoint falls back to including failed rows, and this
  // is the pre-taken-mechanisme inline failure UI.
  it('shows the failure reason for a failed conversion', () => {
    const wrapper = mountStatus([
      { id: '2', target_path: 'brief.md', status: 'failed', error: 'boom' },
    ]);
    const dialog = wrapper.find('nldd-inline-dialog');
    expect(dialog.exists()).toBe(true);
    expect(dialog.attributes('variant')).toBe('alert');
    expect(dialog.attributes('text')).toContain('Brief');
    expect(dialog.attributes('supporting-text')).toBe('boom');
  });
});
