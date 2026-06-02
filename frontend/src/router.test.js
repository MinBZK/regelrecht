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

  it('routes a plain law-id library URL to library (no traject)', () => {
    const r = router.resolve('/library/wet_op_de_zorgtoeslag');
    expect(r.name).toBe('library');
    expect(r.params.lawId).toBe('wet_op_de_zorgtoeslag');
    expect(r.params.trajectRef).toBeUndefined();
  });

  it('mirrors the same split for the editor routes', () => {
    expect(router.resolve(`/editor/${REF}/foo`).name).toBe('editor-traject');
    expect(router.resolve('/editor/wet_op_de_zorgtoeslag').name).toBe('editor');
  });
});

describe('sectionTarget — traject preserved across tab switches', () => {
  it('stamps the active traject when entering a section without one', () => {
    const t = sectionTarget(router, '/library/wet_op_de_zorgtoeslag', REF);
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
    expect(t.name).toBe('library');
    expect(t.params.trajectRef).toBeUndefined();
    expect(t.params.lawId).toBe('foo');
  });
});
