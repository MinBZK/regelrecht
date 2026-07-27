import { describe, it, expect } from 'vitest';
import { trajectSwitchTarget } from './useLastVisitedRoute.js';

const REF = 'ander-traject-12345678';

describe('trajectSwitchTarget', () => {
  it('lands the editor on the bare traject root, dropping the open law', () => {
    // A switch from the editor must not carry the law/article across: the new
    // traject has its own corpus, so the old lawId would point at a document it
    // doesn't have. We navigate to the traject-scoped editor root instead.
    const target = trajectSwitchTarget('editor-traject', REF);
    expect(target).toEqual({ name: 'editor-traject', params: { trajectRef: REF } });
    expect(target.params.lawId).toBeUndefined();
    expect(target.params.articleNumber).toBeUndefined();
  });

  it('lands Home on the bare traject home, dropping the open law', () => {
    // Same rule for the bibliotheek side: go to the traject home root, not the
    // corpus view of a law the new traject may not have.
    const target = trajectSwitchTarget('library-traject', REF);
    expect(target).toEqual({ name: 'traject-home', params: { trajectRef: REF } });
  });

  it('treats every Home-section route name as Home', () => {
    for (const name of ['traject-home', 'corpus-juris', 'werkdocumenten-traject', 'taken-traject']) {
      const target = trajectSwitchTarget(name, REF);
      expect(target.name).toBe('traject-home');
      expect(target.params).toEqual({ trajectRef: REF });
    }
  });
});
