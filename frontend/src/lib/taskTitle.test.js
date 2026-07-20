import { describe, it, expect } from 'vitest';
import { taskTitle, runningTitle } from './taskTitle.js';

// De weergavenaam-resolver zoals useCorpusLaws 'm levert.
const lawName = (id) => (id === 'wet_op_de_zorgtoeslag' ? 'Wet op de zorgtoeslag' : id);

describe('taskTitle', () => {
  it('schrijft een wet-review in de gebiedende wijs, met de weergavenaam', () => {
    const task = {
      task_type: 'job_review',
      title: 'Verrijking beoordelen: wet_op_de_zorgtoeslag',
      payload: { law_id: 'wet_op_de_zorgtoeslag', traject_ref: 't-1' },
    };
    expect(taskTitle(task, lawName)).toBe('Beoordeel verrijking van Wet op de zorgtoeslag');
  });

  it('schrijft een document-review in de gebiedende wijs', () => {
    const task = {
      task_type: 'job_review',
      title: 'Werkdocument beoordelen: mvt.md',
      payload: { kind: 'document', traject_ref: 't-1', target_path: 'mvt.md' },
    };
    expect(taskTitle(task, lawName)).toBe('Beoordeel werkdocument mvt.md');
  });

  it('kort een documentpad in tot de bestandsnaam', () => {
    const task = {
      task_type: 'job_review',
      payload: { kind: 'document', target_path: 'analyses/deel-2/rapport.md' },
    };
    expect(taskTitle(task, lawName)).toBe('Beoordeel werkdocument rapport.md');
  });

  // Een mislukking is geen opdracht: mededelend, niet gebiedend.
  it('meldt een mislukte verrijking, niet gebiedend', () => {
    const task = {
      task_type: 'job_failed',
      title: 'Verrijking mislukt: wet_op_de_zorgtoeslag',
      payload: { law_id: 'wet_op_de_zorgtoeslag', error: 'boom' },
    };
    expect(taskTitle(task, lawName)).toBe('Verrijking van Wet op de zorgtoeslag is mislukt');
  });

  it('meldt een mislukte conversie, ook zonder kind-veld in de payload', () => {
    const task = {
      task_type: 'job_failed',
      title: 'Conversie mislukt: bijlage.md',
      payload: { target_path: 'bijlage.md', error: 'boom' },
    };
    expect(taskTitle(task, lawName)).toBe('Conversie van bijlage.md is mislukt');
  });

  it('valt terug op de servertitel bij een onbekende payload-vorm', () => {
    const task = { task_type: 'job_review', title: 'Iets nieuws: xyz', payload: {} };
    expect(taskTitle(task, lawName)).toBe('Iets nieuws: xyz');
  });

  it('geeft een lege string als er niets te tonen valt', () => {
    expect(taskTitle({ payload: {} })).toBe('');
    expect(taskTitle(undefined)).toBe('');
  });

  it('valt zonder resolver terug op het rauwe law_id', () => {
    const task = { task_type: 'job_review', payload: { law_id: 'kieswet' } };
    expect(taskTitle(task)).toBe('Beoordeel verrijking van kieswet');
  });
});

describe('runningTitle', () => {
  it('schrijft een lopende verrijking als wachten, met de weergavenaam', () => {
    const job = { job_id: 'j1', job_type: 'enrich', law_id: 'wet_op_de_zorgtoeslag' };
    expect(runningTitle(job, lawName)).toBe('Wachten op verrijking van Wet op de zorgtoeslag');
  });

  it('schrijft een lopende conversie als wachten, met de bestandsnaam', () => {
    const job = {
      job_id: 'j2',
      job_type: 'document_convert',
      law_id: 'doc:t-1/analyses/rapport.md',
      target_path: 'analyses/rapport.md',
    };
    expect(runningTitle(job, lawName)).toBe('Wachten op conversie van rapport.md');
  });

  it('valt terug op een generieke naam als een conversie geen target_path heeft', () => {
    const job = { job_id: 'j3', job_type: 'document_convert' };
    expect(runningTitle(job, lawName)).toBe('Wachten op conversie van werkdocument');
  });
});
