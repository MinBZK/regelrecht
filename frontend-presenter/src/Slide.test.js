// @vitest-environment jsdom
import path from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { indexCorpus, lookupArticle } from '../server/presenterApi.js';
import Slide from './Slide.vue';

// The wet block fetches from the dev server; here it reads the real corpus.
vi.mock('./lib/api.js', () => {
  let index;
  return {
    fetchArticle: async (q) => {
      index ??= indexCorpus([path.resolve(import.meta.dirname, '../../corpus/regulation')]);
      return lookupArticle(index, q);
    },
    clearWetCache: () => {},
  };
});

const opts = { global: { config: { compilerOptions: { isCustomElement: (t) => t.startsWith('nldd-') } } } };

describe('Slide', () => {
  it('renders a title slide with an editable presenter field', async () => {
    const src = '---\nkind: title\n---\n\n# Zorgtoeslag\n\nOndertitel met **nadruk**\n';
    const w = mount(Slide, { ...opts, props: { src, deckMeta: { presenter: 'A', affiliation: 'RegelRecht' } } });
    expect(w.find('h1.title-hero').text()).toBe('Zorgtoeslag');
    expect(w.find('.lead-hero').html()).toContain('<strong>nadruk</strong>');
    const input = w.find('input.presenter');
    expect(input.element.value).toBe('A');
    input.element.value = 'Jan Jansen';
    await input.trigger('change');
    expect(w.emitted('meta')).toEqual([['presenter', 'Jan Jansen']]);
  });

  it('renders a wet block from the corpus in words', async () => {
    const src = "# Art 2\n\n```wet\nlaw: wet_op_de_zorgtoeslag\narticle: '2'\ndate: 2025-01-01\nshow: [tekst, definities, invoer, uitvoer, regels]\nleden: [1]\n```\n";
    const w = mount(Slide, { ...opts, props: { src } });
    await flushPromises();
    const card = w.find('.wet');
    expect(card.find('.wet-law').text()).toBe('Wet op de zorgtoeslag');
    // Genummerde leden worden een genummerde lijst; alleen lid 1 is gevraagd.
    expect(card.find('.wet-text').text()).toMatch(/^Indien de normpremie/);
    expect(card.find('.wet-text').text()).not.toContain('De normpremie bedraagt');
    expect(card.text()).toContain('Vaste waarden');
    expect(card.text()).toMatch(/€\s28\.200,96/);
    expect(card.text()).toContain('zorgverzekeringswet → is verzekerd');
    expect(card.findAll('.rule').length).toBeGreaterThan(0);
  });

  it('turns a block into its markdown source in edit mode and emits the edit', async () => {
    const src = '---\nkind: content\n---\n\n# Titel\n\nEen alinea.\n';
    const w = mount(Slide, { ...opts, props: { src, editing: true } });
    const para = w.findAll('.block-editable').find((b) => b.text() === 'Een alinea.');
    await para.trigger('click');
    const area = w.find('textarea.block-editor');
    expect(area.element.value).toBe('Een alinea.');
    await area.setValue('Een andere alinea.');
    await area.trigger('blur');
    const [[block, text]] = w.emitted('replace');
    expect(src.slice(block.start, block.end).trim()).toBe('Een alinea.');
    expect(text).toBe('Een andere alinea.');
  });

  it('renders one line per paragraph on a statement slide', () => {
    const w = mount(Slide, { ...opts, props: { src: '---\nkind: statement\n---\n\nEen **twee**.\n\nDrie.\n' } });
    expect(w.findAll('.statement-line').map((l) => l.text())).toEqual(['Een twee.', 'Drie.']);
  });

  it('shows only the anchored rule, lights up a name and warns about a typo', async () => {
    const src = "# Art 2\n\n```wet\nlaw: wet_op_de_zorgtoeslag\narticle: '2'\ndate: 2025-01-01\nuitvoer: heeft_recht_op_zorgtoeslag\nmarkeer: [vermogen_onder_grens, vermogen_ondr_grens]\n```\n";
    const w = mount(Slide, { ...opts, props: { src } });
    await flushPromises();
    const card = w.find('.wet');
    expect(card.find('.wet-text').exists()).toBe(false);
    expect(card.findAll('.rule').map((r) => r.find('.rule-out').text())).toEqual(['heeft recht op zorgtoeslag']);
    expect(card.findAll('.rule-tree li.lit').map((li) => li.text())).toEqual([expect.stringContaining('vermogen onder grens')]);
    expect(card.find('.wet-warnings').text()).toBe('markeer "vermogen_ondr_grens" komt in dit artikel niet voor');
  });
});
