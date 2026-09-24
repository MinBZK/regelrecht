import { describe, it, expect, vi } from 'vitest';
import { isChunkLoadError, reloadOnStaleBundle } from './reloadOnStaleBundle.js';

function fakeRouter() {
  const handlers = [];
  return {
    onError: (h) => handlers.push(h),
    resolve: (to) => ({ href: `/terugbetaalregimes${to.fullPath}` }),
    fail: (error, to) => handlers.forEach((h) => h(error, to)),
  };
}

function fakeTarget() {
  return { location: { assign: vi.fn(), reload: vi.fn() } };
}

function fakeStorage() {
  const m = new Map();
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => m.set(k, v) };
}

const chromeError = new TypeError('Failed to fetch dynamically imported module: https://x/assets/BeleidView-abc.js');

describe('isChunkLoadError', () => {
  it('recognises the wording of each browser', () => {
    expect(isChunkLoadError(chromeError)).toBe(true);
    expect(isChunkLoadError(new TypeError('error loading dynamically imported module'))).toBe(true);
    expect(isChunkLoadError(new TypeError('Importing a module script failed.'))).toBe(true);
    // Vite throws this when a view's stylesheet is gone; without a
    // vite:preloadError listener it reaches the router like the others.
    expect(isChunkLoadError(new Error('Unable to preload CSS for /terugbetaalregimes/assets/BeleidView-abc.css'))).toBe(true);
  });

  it('leaves other errors alone', () => {
    expect(isChunkLoadError(new Error('Cannot read properties of undefined'))).toBe(false);
    expect(isChunkLoadError(undefined)).toBe(false);
  });
});

describe('reloadOnStaleBundle', () => {
  it('reloads to the page the user was navigating to', () => {
    const router = fakeRouter();
    const target = fakeTarget();
    reloadOnStaleBundle(router, { target, storage: fakeStorage(), now: () => 100_000 });

    router.fail(chromeError, { fullPath: '/beleid' });

    expect(target.location.assign).toHaveBeenCalledWith('/terugbetaalregimes/beleid');
  });

  it('ignores router errors that are not about a missing chunk', () => {
    const router = fakeRouter();
    const target = fakeTarget();
    reloadOnStaleBundle(router, { target, storage: fakeStorage(), now: () => 100_000 });

    router.fail(new Error('boom'), { fullPath: '/beleid' });

    expect(target.location.assign).not.toHaveBeenCalled();
  });

  it('reloads at most once per ten seconds, so a dead server does not loop', () => {
    const router = fakeRouter();
    const target = fakeTarget();
    const storage = fakeStorage();
    let t = 100_000;
    reloadOnStaleBundle(router, { target, storage, now: () => t });

    router.fail(chromeError, { fullPath: '/beleid' });
    t += 5_000;
    router.fail(chromeError, { fullPath: '/beleid' });
    expect(target.location.assign).toHaveBeenCalledTimes(1);

    t += 6_000;
    router.fail(chromeError, { fullPath: '/beleid' });
    expect(target.location.assign).toHaveBeenCalledTimes(2);
  });

  it('still reloads when storage is unavailable', () => {
    const router = fakeRouter();
    const target = fakeTarget();
    const broken = { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('denied'); } };
    reloadOnStaleBundle(router, { target, storage: broken, now: () => 100_000 });

    router.fail(chromeError, { fullPath: '/beleid' });

    expect(target.location.assign).toHaveBeenCalledTimes(1);
  });
});
