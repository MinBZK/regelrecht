import { describe, it, expect } from 'vitest';
import router from '../router.js';
import { reviewTarget } from './taskReview.js';

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
      '/editor/mijn-traject-1a2b3c4d/wet_op_de_zorgtoeslag?task=t1',
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
