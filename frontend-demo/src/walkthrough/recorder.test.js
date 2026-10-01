import { describe, expect, it } from 'vitest';
import { newTakeId, relativePoint } from './recorder.js';
import { parseTakePath } from '../../scripts/walkthrough-dev-server.mjs';
import { mediaFiles } from '../../scripts/fetch-walkthrough-media.mjs';

describe('recorder helpers', () => {
  it('names a take by its local time, safe as a directory', () => {
    expect(newTakeId(new Date(2026, 9, 2, 9, 5, 7))).toBe('2026-10-02T09-05-07');
  });

  it('stores a click as a fraction of the captured area, and ignores the rail', () => {
    const rect = { left: 576, top: 0, width: 1024, height: 1000 };
    expect(relativePoint(rect, 1088, 500)).toEqual({ x: 0.5, y: 0.5 });
    expect(relativePoint(rect, 100, 500)).toBeNull();
    expect(relativePoint({ left: 0, top: 0, width: 0, height: 0 }, 1, 1)).toBeNull();
  });
});

describe('take endpoint', () => {
  it('accepts the four files of a take, appending or replacing', () => {
    expect(parseTakePath('/__walkthrough/takes/2026-10-02T09-05-07/app.webm?mode=append')).toEqual({ take: '2026-10-02T09-05-07', file: 'app.webm', mode: 'append' });
    expect(parseTakePath('/__walkthrough/takes/t1/events.json')?.mode).toBe('replace');
  });

  it('refuses a path that could write outside the take directory', () => {
    expect(parseTakePath('/__walkthrough/takes/../app.webm')).toBeNull();
    expect(parseTakePath('/__walkthrough/takes/%2E%2E/app.webm')).toBeNull();
    expect(parseTakePath('/__walkthrough/takes/t1/..%2Fx.webm')).toBeNull();
    expect(parseTakePath('/__walkthrough/takes/t1/script.js')).toBeNull();
  });
});

describe('components', () => {
  // The production build leaves the recorder panel out (it is dev-only), so
  // nothing else would notice a panel that no longer compiles.
  it('compiles the recorder panel and the player', async () => {
    expect((await import('./RecorderPanel.vue')).default).toBeTruthy();
    expect((await import('./WalkthroughView.vue')).default).toBeTruthy();
  });
});

describe('media fetch', () => {
  it('lists every video and bubble the timeline names, with its checksum', () => {
    const timeline = {
      main: { video: { src: 'main-a.mp4', sha256: 'a', width: 1 }, cam: { src: 'main-cam-b.mp4', sha256: 'b' } },
      faq: [{ video: { src: 'faq-c.mp4', sha256: 'c' }, cam: null }],
    };
    expect(mediaFiles(timeline)).toEqual([
      { src: 'main-a.mp4', sha256: 'a' },
      { src: 'main-cam-b.mp4', sha256: 'b' },
      { src: 'faq-c.mp4', sha256: 'c' },
    ]);
  });
});
