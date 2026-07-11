import { describe, it, expect } from 'vitest';
import router from './router.js';
import { sectionTarget } from './composables/useLastVisitedRoute.js';

// A valid traject ref is `{slug}-{8hex}` (see the route regex). A plain
// law `$id` uses underscores and must NOT match the traject route.
const REF = 'mijn-traject-1a2b3c4d';

describe('route disambiguation (traject vs no-traject)', () => {
  it('routes a {slug}-{8hex} library URL to library-traject', () => {
    const r = router.resolve(`/library/${REF}/zorgtoeslagwet/3`);
    expect(r.name).toBe('library-traject');
    expect(r.params.trajectRef).toBe(REF);
    expect(r.params.lawId).toBe('zorgtoeslagwet');
    expect(r.params.articleNumber).toBe('3');
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

  it('mirrors the same split for the editor routes', () => {
    expect(router.resolve(`/editor/${REF}/foo`).name).toBe('editor-traject');
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
});

describe('sectionTarget — traject preserved across tab switches', () => {
  it('stamps the active traject when entering a section without one', () => {
    const t = sectionTarget(router, '/corpus-juris/wet_op_de_zorgtoeslag', REF);
    expect(t.name).toBe('library-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBe('wet_op_de_zorgtoeslag');
  });

  it('re-stamps a stale stored traject with the currently active one', () => {
    const t = sectionTarget(router, '/editor/old-deadbeef/foo', REF);
    expect(t.name).toBe('editor-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBe('foo');
  });

  it('strips the traject when none is active (Geen traject)', () => {
    const t = sectionTarget(router, `/library/${REF}/foo`, null);
    expect(t.name).toBe('corpus-juris');
    expect(t.params.trajectRef).toBeUndefined();
    expect(t.params.lawId).toBe('foo');
  });

  it('sends the Editor tab to the chooser when no traject is active', () => {
    // The editor requires a traject: with none active, the stored editor
    // path collapses to the chooser and the law travels along as query.
    const t = sectionTarget(router, `/editor/${REF}/foo/3`, null);
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
    expect(sectionTarget(router, '/totally/unknown', REF).name).toBe('library-traject');
    expect(sectionTarget(router, '/totally/unknown', null).name).toBe('home');
    // Section is derived from the path prefix so the right tab is kept.
    expect(sectionTarget(router, '/editor-bogus/x', null).name).toBe('editor');
  });
});
