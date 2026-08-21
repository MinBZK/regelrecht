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

  it('neemt het artikel uit de payload op in de editor-route', () => {
    const task = {
      id: 't1a',
      task_type: 'job_review',
      payload: {
        traject_ref: 'mijn-traject-1a2b3c4d',
        law_id: 'wet_op_de_zorgtoeslag',
        article: '2',
      },
    };
    const resolved = router.resolve(reviewTarget(task));
    expect(resolved.params.articleNumber).toBe('2');
    expect(resolved.fullPath).toBe(
      '/trajecten/mijn-traject-1a2b3c4d/editor/wet_op_de_zorgtoeslag/2?task=t1a',
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

  it('bouwt de werkdocumenten-route met ?task= voor een document-review-taak', () => {
    const task = {
      id: 't5',
      task_type: 'job_review',
      payload: {
        kind: 'document',
        traject_ref: 'mijn-traject-1a2b3c4d',
        target_path: 'bijv-rapport.md',
      },
    };
    const target = reviewTarget(task);
    expect(target).not.toBeNull();
    const resolved = router.resolve(target);
    expect(resolved.name).toBe('werkdocumenten-traject');
    expect(resolved.params.trajectRef).toBe('mijn-traject-1a2b3c4d');
    expect(resolved.params.docPath).toBe('bijv-rapport.md');
    expect(resolved.fullPath).toBe(
      '/trajecten/mijn-traject-1a2b3c4d/werkdocumenten/bijv-rapport.md?task=t5',
    );
  });

  it('geeft null voor een document-review-taak zonder target_path', () => {
    expect(
      reviewTarget({
        id: 't6',
        payload: { kind: 'document', traject_ref: 'mijn-traject-1a2b3c4d' },
      }),
    ).toBeNull();
  });

  it('geeft null voor een document-review-taak zonder traject_ref', () => {
    expect(
      reviewTarget({ id: 't7', payload: { kind: 'document', target_path: 'bijv-rapport.md' } }),
    ).toBeNull();
  });

  it('routeert een law_create-taak naar de editor-traject-route (wet-branch)', () => {
    const task = {
      id: 't8',
      task_type: 'job_review',
      payload: {
        kind: 'law_create',
        traject_ref: 'mijn-traject-1a2b3c4d',
        law_id: 'werkinstructie_toetsing',
      },
    };
    const target = reviewTarget(task);
    expect(target).not.toBeNull();
    const resolved = router.resolve(target);
    expect(resolved.name).toBe('editor-traject');
    expect(resolved.params.lawId).toBe('werkinstructie_toetsing');
    expect(resolved.fullPath).toBe(
      '/trajecten/mijn-traject-1a2b3c4d/editor/werkinstructie_toetsing?task=t8',
    );
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

  it('pakt het aangewezen artikel, ook als een eerder artikel ook afwijkt', () => {
    const current = [
      { number: '1', text: 'oud-1' },
      { number: '2', text: 'oud-2' },
    ];
    const proposed = [
      { number: '1', text: 'nieuw-1' },
      { number: '2', text: 'nieuw-2' },
    ];
    const result = proposalDivergence(current, proposed, '2');
    expect(result.target).toEqual(proposed[1]);
    // Artikel 1 hangt aan zijn eigen taak, dus het is hier geen verborgen
    // wijziging waar de banner naar hoeft te verwijzen.
    expect(result.hiddenChanges).toBe(false);
  });

  it('geeft geen target als het aangewezen artikel niet afwijkt', () => {
    const current = [{ number: '1', text: 'zelfde' }];
    const proposed = [{ number: '1', text: 'zelfde' }];
    expect(proposalDivergence(current, proposed, '1')).toEqual({
      target: null,
      hiddenChanges: false,
    });
  });

  it('geeft geen target als het aangewezen artikel in de wet of het voorstel ontbreekt', () => {
    const current = [{ number: '1', text: 'oud' }];
    const proposed = [{ number: '1', text: 'nieuw' }];
    expect(proposalDivergence(current, proposed, '9')).toEqual({
      target: null,
      hiddenChanges: false,
    });
    expect(proposalDivergence([], proposed, '1')).toEqual({
      target: null,
      hiddenChanges: false,
    });
  });

  it('vergelijkt het nummer als tekst, zodat een numeriek payload-artikel raak is', () => {
    const current = [{ number: '2', text: 'oud' }];
    const proposed = [{ number: 2, text: 'nieuw' }];
    expect(proposalDivergence(current, proposed, 2).target).toEqual(proposed[0]);
  });
});
