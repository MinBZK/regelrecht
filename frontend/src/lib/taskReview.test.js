import { describe, it, expect } from 'vitest';
import router from '../router.js';
import { reviewTarget, proposalDivergence } from './taskReview.js';

describe('reviewTarget', () => {
  it('bouwt de editor-traject-route met ?task= voor een job_review-taak', () => {
    const task = {
      id: 't1',
      task_type: 'job_review',
      payload: { traject_ref: 'mijn-traject-1a2b3c4d', law_id: 'wet_op_de_zorgtoeslag' },
    };
    const target = reviewTarget(task);
    expect(target).not.toBeNull();
    const resolved = router.resolve(target);
    expect(resolved.name).toBe('editor-traject');
    expect(resolved.params.trajectRef).toBe('mijn-traject-1a2b3c4d');
    expect(resolved.params.lawId).toBe('wet_op_de_zorgtoeslag');
    expect(resolved.fullPath).toBe(
      '/trajecten/mijn-traject-1a2b3c4d/editor/wet_op_de_zorgtoeslag?task=t1',
    );
  });

  it('geeft null voor een taak zonder traject_ref in de payload', () => {
    expect(reviewTarget({ id: 't2', payload: { law_id: 'wet_op_de_zorgtoeslag' } })).toBeNull();
  });

  it('geeft null voor een taak zonder law_id in de payload', () => {
    expect(reviewTarget({ id: 't3', payload: { traject_ref: 'mijn-traject-1a2b3c4d' } })).toBeNull();
  });

  it('geeft null voor een taak zonder payload', () => {
    expect(reviewTarget({ id: 't4' })).toBeNull();
  });
});

describe('proposalDivergence', () => {
  it('vindt het enige afwijkende artikel als target zonder hidden changes', () => {
    const current = [{ number: '1', text: 'oud' }];
    const proposed = [{ number: '1', text: 'nieuw' }];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toEqual(proposed[0]);
    expect(result.hiddenChanges).toBe(false);
  });

  it('geeft geen target als niets afwijkt', () => {
    const current = [{ number: '1', text: 'zelfde' }];
    const proposed = [{ number: '1', text: 'zelfde' }];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toBeNull();
    expect(result.hiddenChanges).toBe(false);
  });

  it('markeert een tweede afwijkend artikel als hidden change', () => {
    const current = [
      { number: '1', text: 'oud-1' },
      { number: '2', text: 'oud-2' },
    ];
    const proposed = [
      { number: '1', text: 'nieuw-1' },
      { number: '2', text: 'nieuw-2' },
    ];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toEqual(proposed[0]);
    expect(result.hiddenChanges).toBe(true);
  });

  it('markeert een voorgesteld artikel dat de huidige wet niet heeft als hidden change', () => {
    const current = [{ number: '1', text: 'oud' }];
    const proposed = [
      { number: '1', text: 'oud' },
      { number: '2', text: 'nieuw artikel' },
    ];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toBeNull();
    expect(result.hiddenChanges).toBe(true);
  });

  it('markeert een artikel-verwijdering (huidige wet heeft artikel 2, voorstel niet) als hidden change', () => {
    const current = [
      { number: '1', text: 'blijft' },
      { number: '2', text: 'wordt verwijderd' },
    ];
    const proposed = [{ number: '1', text: 'blijft' }];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toBeNull();
    expect(result.hiddenChanges).toBe(true);
  });

  it('behandelt lege/ontbrekende input als geen divergentie', () => {
    expect(proposalDivergence(undefined, undefined)).toEqual({
      target: null,
      hiddenChanges: false,
    });
    expect(proposalDivergence([], [])).toEqual({ target: null, hiddenChanges: false });
  });
});
