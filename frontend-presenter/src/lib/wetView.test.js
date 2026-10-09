// @vitest-environment node
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { indexCorpus, lookupArticle } from '../../server/presenterApi.js';
import { directRefs, sections, selectArticle } from './wetView.js';

const index = indexCorpus([path.resolve(import.meta.dirname, '../../../corpus/regulation')]);
const { article } = lookupArticle(index, { law: 'wet_op_de_zorgtoeslag', article: '2', date: '2025-01-01' });

describe('sections', () => {
  it('shows the text when nothing is asked for', () => {
    expect([...sections({})]).toEqual(['tekst']);
  });

  it('lets an anchor bring its own section', () => {
    expect([...sections({ uitvoer: 'x' })]).toEqual(['regels']);
    expect([...sections({ invoer: ['x'], leden: [1] })].sort()).toEqual(['invoer', 'tekst']);
  });

  it('lets an explicit show win over the anchors', () => {
    expect([...sections({ show: ['uitvoer'], uitvoer: 'x' })]).toEqual(['uitvoer']);
  });
});

describe('selectArticle', () => {
  it('keeps everything without anchors', () => {
    const v = selectArticle(article, {});
    expect(v.actions).toHaveLength(article.machine_readable.execution.actions.length);
    expect(v.unknown).toEqual([]);
  });

  it('narrows the rules and outputs to one output by name', () => {
    const v = selectArticle(article, { uitvoer: 'heeft_recht_op_zorgtoeslag' });
    expect(v.actions.map((a) => a.output)).toEqual(['heeft_recht_op_zorgtoeslag']);
    expect(v.outputs.map((o) => o.name)).toEqual(['heeft_recht_op_zorgtoeslag']);
  });

  it('narrows inputs, and counts a parameter as an input', () => {
    const v = selectArticle(article, { invoer: ['bsn', 'toetsingsinkomen'] });
    expect(v.inputs.map((i) => i.name)).toEqual(['bsn', 'toetsingsinkomen']);
    expect(v.inputs[0].isParameter).toBe(true);
  });

  it('reports names the article does not have, for every kind of anchor', () => {
    const v = selectArticle(article, { uitvoer: 'hoogte_zorgtoesla', invoer: ['toetsinginkomen'], definities: 'nee', markeer: ['bestaat_niet', 'standaardpremie'] });
    expect(v.unknown).toEqual([
      { kind: 'definitie', name: 'nee' },
      { kind: 'invoer', name: 'toetsinginkomen' },
      { kind: 'uitvoer', name: 'hoogte_zorgtoesla' },
      { kind: 'markeer', name: 'bestaat_niet' },
    ]);
  });

  it('survives an article without machine_readable', () => {
    expect(selectArticle({ number: '6', text: 'x' }, { uitvoer: 'y' })).toMatchObject({ actions: [], outputs: [], unknown: [{ kind: 'uitvoer', name: 'y' }] });
  });
});

describe('directRefs', () => {
  it('finds the $names on a node, not in its children', () => {
    const node = { operation: 'AND', conditions: ['$a', { operation: 'EQUALS', subject: '$b', value: true }], values: ['$c', 3] };
    expect([...directRefs(node)].sort()).toEqual(['a', 'c']);
    expect([...directRefs(node.conditions[1])]).toEqual(['b']);
  });
});
