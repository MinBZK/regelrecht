import { describe, it, expect } from 'vitest';
import router from './router.js';
import { sectionTarget } from './composables/useLastVisitedRoute.js';

// A valid traject ref is `{slug}-{8hex}` (see the route regex). A plain
// law `$id` uses underscores and must NOT match the traject route.
const REF = 'mijn-traject-1a2b3c4d';

describe('route disambiguation (traject vs no-traject)', () => {
  it('routes a bare {slug}-{8hex} traject URL to traject-home', () => {
    const r = router.resolve(`/trajecten/${REF}`);
    expect(r.name).toBe('traject-home');
    expect(r.params.trajectRef).toBe(REF);
    expect(r.params.lawId).toBeUndefined();
  });

  it('routes a traject corpus URL to library-traject', () => {
    const r = router.resolve(`/trajecten/${REF}/corpus/zorgtoeslagwet/3`);
    expect(r.name).toBe('library-traject');
    expect(r.params.trajectRef).toBe(REF);
    expect(r.params.lawId).toBe('zorgtoeslagwet');
    expect(r.params.articleNumber).toBe('3');
  });

  it('routes a traject werkdocumenten URL to werkdocumenten-traject', () => {
    const r = router.resolve(`/trajecten/${REF}/werkdocumenten/map/besluit.md`);
    expect(r.name).toBe('werkdocumenten-traject');
    expect(r.params.trajectRef).toBe(REF);
    expect(r.params.docPath).toBe('map/besluit.md');
  });

  it('redirects the old standalone werkdocumenten URL into Home', () => {
    const matched = router.resolve(`/werkdocumenten/${REF}/besluit.md`).matched;
    const redirect = matched[matched.length - 1].redirect;
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { trajectRef: REF, docPath: 'besluit.md' } })).toMatchObject({
      name: 'werkdocumenten-traject',
      params: { trajectRef: REF, docPath: 'besluit.md' },
    });
  });

  it('routes a plain law-id corpus URL to corpus-juris (no traject)', () => {
    const r = router.resolve('/corpus-juris/wet_op_de_zorgtoeslag');
    expect(r.name).toBe('corpus-juris');
    expect(r.params.lawId).toBe('wet_op_de_zorgtoeslag');
    expect(r.params.trajectRef).toBeUndefined();
  });

  it('serves Home at the clean root and redirects the old public library URLs', () => {
    expect(router.resolve('/').name).toBe('home');
    const matched = router.resolve('/library/wet_op_de_zorgtoeslag').matched;
    const redirect = matched[matched.length - 1].redirect;
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { lawId: 'wet_op_de_zorgtoeslag' } })).toMatchObject({
      name: 'corpus-juris',
      params: { lawId: 'wet_op_de_zorgtoeslag' },
    });
  });

  it('redirects the old traject library URLs onto /trajecten/{ref}', () => {
    // Bare old traject library -> traject-home.
    let matched = router.resolve(`/library/${REF}`).matched;
    let redirect = matched[matched.length - 1].redirect;
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { trajectRef: REF } })).toMatchObject({
      name: 'traject-home',
      params: { trajectRef: REF },
    });
    // With a law -> library-traject (the /corpus/{law} segment).
    matched = router.resolve(`/library/${REF}/zorgtoeslagwet/3`).matched;
    redirect = matched[matched.length - 1].redirect;
    expect(redirect).toBeTruthy();
    expect(
      redirect({ params: { trajectRef: REF, lawId: 'zorgtoeslagwet', articleNumber: '3' } }),
    ).toMatchObject({
      name: 'library-traject',
      params: { trajectRef: REF, lawId: 'zorgtoeslagwet' },
    });
  });

  it('mirrors the same split for the editor routes', () => {
    expect(router.resolve(`/trajecten/${REF}/editor/foo`).name).toBe('editor-traject');
    // The chooser lives at its own section-neutral URL (/trajecten); the bare
    // /editor redirects onto it with sectie=editor, and a no-traject law URL
    // does the same with the law carried as query.
    expect(router.resolve('/trajecten').name).toBe('trajecten');
    expect(router.resolve('/editor/nieuw-traject').name).toBe('editor-nieuw-traject');
    // A no-traject law URL hits a redirect route (router.resolve returns the
    // redirect record without following it). Assert it redirects onto the
    // chooser, carrying the section + law as query.
    const matched = router.resolve('/editor/wet_op_de_zorgtoeslag').matched;
    const redirect = matched[matched.length - 1].redirect;
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { lawId: 'wet_op_de_zorgtoeslag' } })).toMatchObject({
      name: 'trajecten',
      query: { sectie: 'editor', law: 'wet_op_de_zorgtoeslag' },
    });
  });

  it('redirects the old traject editor URLs onto /trajecten/{ref}/editor', () => {
    const matched = router.resolve(`/editor/${REF}/foo`).matched;
    const redirect = matched[matched.length - 1].redirect;
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { trajectRef: REF, lawId: 'foo' } })).toMatchObject({
      name: 'editor-traject',
      params: { trajectRef: REF, lawId: 'foo' },
    });
  });
});

describe('sectionTarget - traject preserved across tab switches', () => {
  it('stamps the active traject when entering a section without one', () => {
    const t = sectionTarget(router, '/corpus-juris/wet_op_de_zorgtoeslag', REF);
    expect(t.name).toBe('library-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBe('wet_op_de_zorgtoeslag');
  });

  it('re-stamps a stale stored traject with the currently active one', () => {
    const t = sectionTarget(router, '/trajecten/old-deadbeef/editor/foo', REF);
    expect(t.name).toBe('editor-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBe('foo');
  });

  it('strips the traject when none is active (Geen traject)', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/corpus/foo`, null);
    expect(t.name).toBe('corpus-juris');
    expect(t.params.trajectRef).toBeUndefined();
    expect(t.params.lawId).toBe('foo');
  });

  it('preserves the werkdocumenten sub-mode across a tab switch', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/werkdocumenten/besluit.md`, REF);
    expect(t.name).toBe('werkdocumenten-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.docPath).toBe('besluit.md');
  });

  it('re-stamps the active traject onto a stored werkdocumenten path', () => {
    const t = sectionTarget(router, '/trajecten/old-deadbeef/werkdocumenten/x.md', REF);
    expect(t.name).toBe('werkdocumenten-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.docPath).toBe('x.md');
  });

  it('drops the werkdocumenten sub-mode when no traject is active', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/werkdocumenten/x.md`, null);
    expect(t.name).toBe('home');
  });

  it('sends the Editor tab to the chooser when no traject is active', () => {
    // The editor requires a traject: with none active, the stored editor
    // path collapses to the chooser and the law travels along as query.
    const t = sectionTarget(router, `/trajecten/${REF}/editor/foo/3`, null);
    expect(t.name).toBe('editor');
    expect(t.params.trajectRef).toBeUndefined();
    expect(t.query).toEqual({ law: 'foo', article: '3' });
  });

  it('upgrades a stored chooser path to the active traject editor', () => {
    const t = sectionTarget(router, '/editor?law=foo', REF);
    expect(t.name).toBe('editor-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBe('foo');
  });

  it('falls back to the section root for an unresolvable stored path', () => {
    // A corrupted/stale sessionStorage value must not crash router.push.
    expect(sectionTarget(router, '/totally/unknown', REF).name).toBe('traject-home');
    expect(sectionTarget(router, '/totally/unknown', null).name).toBe('home');
    // Section is derived from the path prefix so the right tab is kept.
    expect(sectionTarget(router, '/editor-bogus/x', null).name).toBe('editor');
  });
});
