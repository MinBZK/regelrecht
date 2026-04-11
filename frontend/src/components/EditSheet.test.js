import { describe, it, expect, beforeAll } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import EditSheet from './EditSheet.vue';

// Vite is configured with `isCustomElement: tag => tag.startsWith('ndd-')`
// (vite.config.js), so Vue's compiler emits raw HTML elements for the
// `<ndd-sheet>` tag rather than Vue components. The Vue Test Utils
// `stubs:` map doesn't apply to those — the template ref ends up pointing
// at a real HTMLElement in happy-dom, which has no `show()` / `hide()`
// methods, so EditSheet's `watch(item, ...)` throws an unhandled
// rejection on every mount.
//
// Register a minimal custom element that *does* expose show/hide so the
// watcher runs cleanly. We don't care about visual rendering — the tests
// drive the component via `wrapper.vm.values` / `wrapper.vm.save()`.
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('ndd-sheet')) {
    class NddSheetTestStub extends HTMLElement {
      show() {}
      hide() {}
    }
    customElements.define('ndd-sheet', NddSheetTestStub);
  }
});

/**
 * EditSheet relies on `ndd-sheet`'s `show()` / `hide()` methods which only
 * exist on the real custom element. In jsdom there is no element backing
 * those methods, so we stub the ref before the watch fires by mounting
 * with a sheetEl shim.
 *
 * Each test mounts EditSheet with `attachTo` so the ref resolution
 * doesn't crash, then sets a `props.item` to drive the watch and exercise
 * each section's data flow.
 */
function mountSheet(item = null) {
  // No stubs needed: ndd-* tags are treated as raw HTML by Vue's compiler
  // (see vite.config.js `isCustomElement`), and `ndd-sheet` is registered
  // above with no-op show/hide so the watcher in EditSheet doesn't crash.
  // The remaining ndd-* elements render as empty <ndd-*> nodes; the tests
  // poke values directly through `wrapper.vm.values` and exercise the
  // emitted save payloads, not the rendered form.
  return mount(EditSheet, {
    attachTo: document.body,
    props: { item },
  });
}

async function setItem(wrapper, item) {
  await wrapper.setProps({ item });
  await nextTick();
}

describe('EditSheet', () => {
  describe('definition', () => {
    it('emits save with renamed key when editing an existing definition', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'definition',
        key: 'old_name',
        rawDef: { value: 100 },
      });

      wrapper.vm.values.name = 'new_name';
      wrapper.vm.save();
      await nextTick();

      const events = wrapper.emitted('save');
      expect(events).toBeTruthy();
      expect(events[0][0]).toMatchObject({
        section: 'definition',
        key: 'old_name',
        newKey: 'new_name',
      });
    });

    it('emits save with add-definition section for new definitions', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-definition',
        isNew: true,
      });

      wrapper.vm.values.name = 'foo';
      wrapper.vm.save();
      await nextTick();

      const events = wrapper.emitted('save');
      expect(events[0][0].section).toBe('add-definition');
      expect(events[0][0].key).toBe('foo');
    });
  });

  describe('parameter', () => {
    it('emits save with parameter data populated from item', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'parameter',
        index: 0,
        data: { name: 'bsn', type: 'string', required: true },
      });

      // Watch should hydrate values from item.data
      expect(wrapper.vm.values).toMatchObject({
        name: 'bsn',
        type: 'string',
        required: true,
      });

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0]).toMatchObject({
        section: 'parameter',
        index: 0,
        data: { name: 'bsn', type: 'string', required: true },
      });
    });
  });

  describe('input', () => {
    it('emits save with source.regulation and source.output', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-input',
        isNew: true,
      });

      // Fill name, sourceRegulation, sourceOutput by mutating the values
      // ref directly — querying inputs by index is brittle when several
      // text-field stubs render.
      wrapper.vm.values.name = 'leeftijd';
      wrapper.vm.values.type = 'number';
      wrapper.vm.values.sourceRegulation = 'wet_basisregistratie_personen';
      wrapper.vm.values.sourceOutput = 'leeftijd';

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0].section).toBe('add-input');
      expect(events[0][0].data).toMatchObject({
        name: 'leeftijd',
        type: 'number',
        source: {
          regulation: 'wet_basisregistratie_personen',
          output: 'leeftijd',
        },
      });
    });

    it('emits save with source.parameters when rows are populated', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-input',
        isNew: true,
      });

      wrapper.vm.values.name = 'leeftijd';
      wrapper.vm.values.sourceRegulation = 'wet_basisregistratie_personen';
      wrapper.vm.values.sourceOutput = 'leeftijd';
      wrapper.vm.values.sourceParameters = [
        { key: 'bsn', value: '$bsn' },
        { key: 'peildatum', value: '2025-01-01' },
      ];

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0].data.source).toMatchObject({
        regulation: 'wet_basisregistratie_personen',
        output: 'leeftijd',
        parameters: { bsn: '$bsn', peildatum: '2025-01-01' },
      });
    });

    it('hydrates source.parameters from existing data on edit', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          type: 'number',
          source: {
            regulation: 'wet_basisregistratie_personen',
            output: 'leeftijd',
            parameters: { bsn: '$bsn', peildatum: '2025-01-01' },
          },
        },
      });

      // Each row also carries a stable `_rowId` so v-for keys survive
      // deletion; assert key/value pairs ignoring the auxiliary id.
      const rows = wrapper.vm.values.sourceParameters.map(
        ({ key, value }) => ({ key, value }),
      );
      expect(rows).toEqual([
        { key: 'bsn', value: '$bsn' },
        { key: 'peildatum', value: '2025-01-01' },
      ]);
    });

    it('skips non-scalar source.parameter values from the editable rows but preserves them on save', async () => {
      const wrapper = mountSheet();
      const warnings = [];
      const origWarn = console.warn;
      console.warn = (msg) => warnings.push(String(msg));

      try {
        await setItem(wrapper, {
          section: 'input',
          index: 0,
          data: {
            name: 'leeftijd',
            source: {
              parameters: {
                bsn: '$bsn',
                nested: { foo: 'bar' }, // unsupported in form editor
              },
            },
          },
        });
      } finally {
        console.warn = origWarn;
      }

      const rows = wrapper.vm.values.sourceParameters.map(({ key }) => key);
      expect(rows).toEqual(['bsn']);
      expect(warnings.some((m) => m.includes('nested'))).toBe(true);

      // The non-scalar key must round-trip on save: the form doesn't
      // render it, but it should still appear in the emitted payload
      // unchanged.
      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0].data.source.parameters).toEqual({
        bsn: '$bsn',
        nested: { foo: 'bar' },
      });
    });

    it('round-trips numeric source.parameter values without stringifying them', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          source: {
            regulation: 'wet_basisregistratie_personen',
            output: 'leeftijd',
            parameters: { threshold: 42, enabled: true, bsn: '$bsn' },
          },
        },
      });

      // No edits to value cells; save should preserve original types.
      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0].data.source.parameters).toEqual({
        threshold: 42,
        enabled: true,
        bsn: '$bsn',
      });
    });

    it('emits user-typed source.parameter value as a string when the user touches the row', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          source: { parameters: { threshold: 42 } },
        },
      });

      // Simulate the user editing the value field.
      wrapper.vm.values.sourceParameters[0].value = '99';

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      // User-typed value emits as a string — we don't second-guess what
      // they meant, and the save path treats explicit edits as opaque
      // text.
      expect(events[0][0].data.source.parameters).toEqual({ threshold: '99' });
    });

    it('lets a user-added row override an overflow key with the same name', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          source: {
            parameters: { complex: { foo: 'bar' } },
          },
        },
      });

      // Hydration skipped `complex`; user adds a new row with the same key.
      wrapper.vm.values.sourceParameters.push({ _rowId: 999, key: 'complex', value: 'replaced' });
      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0].data.source.parameters).toEqual({ complex: 'replaced' });
    });

    it('skips source.parameters rows with empty key on save', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-input',
        isNew: true,
      });

      wrapper.vm.values.name = 'foo';
      wrapper.vm.values.sourceRegulation = 'bar';
      wrapper.vm.values.sourceParameters = [
        { key: 'bsn', value: '$bsn' },
        { key: '', value: 'orphan' },
        { key: '   ', value: 'whitespace' },
      ];

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0].data.source.parameters).toEqual({ bsn: '$bsn' });
    });

    it('omits source entirely when no regulation/output/parameters set', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-input',
        isNew: true,
      });

      wrapper.vm.values.name = 'foo';
      // Leave source* blank.

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0].data).not.toHaveProperty('source');
    });
  });

  describe('output', () => {
    it('emits save with output data', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'output',
        index: 0,
        data: { name: 'heeft_recht', type: 'boolean' },
      });

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0]).toMatchObject({
        section: 'output',
        index: 0,
        data: { name: 'heeft_recht', type: 'boolean' },
      });
    });

    it('preserves type_spec on amount outputs', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'output',
        index: 0,
        data: {
          name: 'hoogte',
          type: 'amount',
          type_spec: { unit: 'eurocent' },
        },
      });

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(events[0][0].data.type_spec).toEqual({ unit: 'eurocent' });
    });
  });
});
