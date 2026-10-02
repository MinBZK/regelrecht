/**
 * The replay engine: the presenter's voice plays, and at the moments the
 * presenter acted the engine does the same in the live demo.
 *
 * The voice is the clock. Every event in the timeline has a time on it; when
 * the audio reaches that time the event is applied: a slide change through
 * the deck (which opens the tab and the persona, as in a presentation), a
 * click or typing through `locator` and `actions`. An event whose element is
 * not there yet (a tab still loading, the engine still computing) waits for
 * it, and the voice waits along.
 *
 * Jumping does not replay from the start. Each chapter carries the demo state
 * as it was when its slide came up; a jump restores that, mounts the tabs
 * fresh and replays the chapter's events up to the target, fast.
 *
 * The viewer can click along: a trusted click in the demo pauses the replay
 * and leaves the demo to them. Play again and the state of that moment comes
 * back. While it runs, the viewer's own state is set aside and not written
 * to storage; leaving restores it.
 */
import { nextTick, reactive } from 'vue';
import { adoptLocale, currentLocale, t } from '../i18n/index.js';
import { setPersistence } from '../store/demoStore.js';
import { localeRouteName, pageForConfigPath } from '../router.js';
import { click, setChecked, setValue, key as pressKey, scrollTo } from './actions.js';
import { installClock, uninstallClock } from './clock.js';
import { resolve } from './locator.js';
import { CAPTIONS_BASE, MEDIA_BASE, chapterAt, parseVtt } from './timeline.js';
import { resetViews } from './viewEpoch.js';

const SOURCE_LOCALE = 'nl';
const POSITION_KEY = 'rr-walkthrough-position-v2';

export const replay = reactive({
  active: false,
  playing: false,
  now: 0,
  speed: 1,
  captionsOn: true,
  camOn: true,
  waiting: false,
  /** The viewer clicked along; play restores the recorded state first. */
  diverged: false,
  faq: null,
  cues: [],
  cursor: { x: 0, y: 0, visible: false },
  ripples: [],
});

let timeline = null;
let ctx = null; // { router, demo, presentation }
let audio = null;
let next = 0; // index of the next event to apply
/** The seek token of whoever is applying events now (pump or seek), or null. */
let busyOwner = null;
let raf = 0;
let mainPosition = 0;
let backup = null;
let previousLocale = null;
let seekToken = 0;

export function currentTrack() {
  return replay.faq ?? timeline?.main ?? null;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const clone = (v) => JSON.parse(JSON.stringify(v));

function recordedAt(track) {
  const t = Date.parse(track?.recordedAt ?? '');
  return Number.isFinite(t) ? t : null;
}

// ---- the demo's state -----------------------------------------------------------

function applyState(state) {
  if (!state || !ctx) return;
  const { demo } = ctx;
  Object.assign(demo.state, clone(state));
  // The replay always shows the rail, whatever mode the presenter was in.
  demo.state.presentationMode = 'zelfstandig';
  demo.reregister();
}

/** The deck's slides as recorded, with corrections applied by the pipeline. */
function slidesOf(track) {
  return track?.slides?.length ? track.slides : ctx?.demo?.corpus?.value?.config?.slides ?? [];
}

/**
 * The slides a track shows. In an answer every slide carries the question
 * as its title, whatever slide the answer was recorded on: the viewer chose
 * that question and should keep seeing it. Route and persona stay, so the
 * deck still opens the tab the answer talks about.
 */
function slidesFor(track) {
  const slides = slidesOf(track);
  if (!replay.faq || track?.id !== replay.faq.id) return slides;
  return slides.map((s) => ({
    ...s,
    kind: s.route ? s.kind : 'section',
    overline: t('walkthrough.faq.overline'),
    title: replay.faq.question,
    lead: undefined,
    lines: undefined,
    bullets: undefined,
    note: undefined,
  }));
}

function samePage(path) {
  const want = pageForConfigPath(path) ?? path;
  const here = ctx.router.currentRoute.value;
  return (here.meta?.page ?? here.path) === want || here.path === path;
}

// ---- finding and doing --------------------------------------------------------

/**
 * Whether work started under `token` may still touch the demo: not after a
 * jump (a newer token), not after leaving, not once the viewer took over.
 */
function current(token) {
  return token === seekToken && replay.active && !replay.diverged;
}

/**
 * The element of `target`, waiting for it to appear. If the wait gets long
 * the voice stops too, so the picture never runs behind the words. Only when
 * the exact element does not show up is a looser match tried: trying it at
 * once would click a look-alike while the real one is still rendering.
 */
async function find(target, { timeout = 4000, fast = false, token = seekToken } = {}) {
  const started = performance.now();
  const limit = fast ? 1500 : timeout;
  let paused = false;
  for (;;) {
    if (!current(token)) {
      if (paused) replay.waiting = false;
      return null;
    }
    const waited = performance.now() - started;
    const el = resolve(target, { loose: waited > limit / 2 });
    if (el) {
      if (paused) {
        replay.waiting = false;
        if (replay.playing) audio?.play().catch(() => {});
      }
      return el;
    }
    if (waited > limit) break;
    if (!fast && !paused && waited > 250 && replay.playing) {
      paused = true;
      replay.waiting = true;
      audio?.pause();
    }
    await sleep(60);
  }
  if (paused) {
    replay.waiting = false;
    if (replay.playing) audio?.play().catch(() => {});
  }
  return null;
}

function pointIn(el, fx = 0.5, fy = 0.5) {
  const r = el.getBoundingClientRect();
  return { x: r.left + r.width * fx, y: r.top + r.height * fy };
}

function showCursorAt(p) {
  replay.cursor.x = p.x;
  replay.cursor.y = p.y;
  replay.cursor.visible = true;
}

function ripple(p) {
  const id = `${performance.now()}`;
  replay.ripples.push({ id, ...p });
  setTimeout(() => {
    const i = replay.ripples.findIndex((r) => r.id === id);
    if (i >= 0) replay.ripples.splice(i, 1);
  }, 700);
}

async function apply(e, { fast = false, token = seekToken } = {}) {
  const { router, presentation } = ctx;
  const opts = { fast, token };
  switch (e.type) {
    case 'restore':
      applyState(e.state);
      resetViews();
      await nextTick();
      if (e.slideIndex != null) await presentation.goTo(e.slideIndex);
      return;
    case 'slide':
      if (presentation.index.value !== e.index || !presentation.active.value) await presentation.goTo(e.index);
      return;
    case 'route': {
      // Routes follow from clicks. Only when a click did not get there (a
      // control that moved, a tab that is not there) does the replay go
      // itself, so the story stays on the tab the voice talks about.
      const deadline = performance.now() + (fast ? 300 : 1500);
      while (performance.now() < deadline && !samePage(e.path)) {
        await sleep(60);
        if (!current(token)) return;
      }
      if (!samePage(e.path) && current(token)) await router.push(e.path).catch(() => {});
      return;
    }
    case 'click': {
      const el = await find(e.target, opts);
      if (!el || !current(token)) return;
      el.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
      if (!fast) {
        const p = pointIn(el, e.fx, e.fy);
        showCursorAt(p);
        ripple(p);
      }
      click(el, e);
      return;
    }
    case 'input': {
      const el = await find(e.target, opts);
      if (!el || !current(token)) return;
      if (!fast) showCursorAt(pointIn(el, 0.15, 0.5));
      setValue(el, e.value ?? '');
      return;
    }
    case 'change': {
      const el = await find(e.target, opts);
      if (!el || !current(token)) return;
      if (typeof e.checked === 'boolean') setChecked(el, e.checked);
      else setValue(el, e.value ?? '', { change: true });
      return;
    }
    case 'key': {
      const el = e.target ? await find(e.target, { ...opts, timeout: 1000 }) : null;
      if (!current(token)) return;
      if (el) pressKey(el, e.key);
      else document.dispatchEvent(new KeyboardEvent('keydown', { key: e.key, bubbles: true }));
      return;
    }
    case 'scroll': {
      const el = e.target ? await find(e.target, { ...opts, timeout: 1000 }) : document.scrollingElement;
      if (el && current(token)) scrollTo(el, e.top ?? 0, e.left ?? 0);
      return;
    }
    default:
  }
}

// ---- the clock ---------------------------------------------------------------------

/**
 * Move the cursor toward the next click a moment before it happens, so the
 * viewer sees where it goes instead of a ripple out of nowhere.
 */
let leadFor = -1;
function lead(events, now) {
  const e = events[next];
  if (!e || e.type !== 'click' || leadFor === next || e.t - now > 0.45) return;
  leadFor = next;
  const el = resolve(e.target);
  if (el) showCursorAt(pointIn(el, e.fx, e.fy));
}

async function pump() {
  if (busyOwner !== null || replay.diverged) return;
  const track = currentTrack();
  const events = track?.events ?? [];
  const token = seekToken;
  busyOwner = token;
  try {
    while (next < events.length && events[next].t <= replay.now) {
      await apply(events[next], { token });
      if (!current(token)) return; // a jump, a stop or the viewer meanwhile
      next += 1;
    }
    lead(events, replay.now);
  } finally {
    if (busyOwner === token) busyOwner = null;
  }
}

function tick() {
  if (audio) replay.now = audio.currentTime;
  pump();
  raf = replay.playing ? requestAnimationFrame(tick) : 0;
}

function onEnded() {
  if (replay.faq) backToMain();
  else pause();
}

function makeAudio() {
  audio = new Audio();
  audio.preload = 'auto';
  audio.addEventListener('ended', onEnded);
  return audio;
}

async function loadCaptions(track) {
  replay.cues = [];
  const file = track?.captions?.nl;
  if (!file) return;
  try {
    const res = await fetch(`${CAPTIONS_BASE}${file}`);
    if (res.ok) replay.cues = parseVtt(await res.text());
  } catch {
    /* captions are a help, not a condition */
  }
}

async function loadTrack(track) {
  const a = audio ?? makeAudio();
  const src = `${MEDIA_BASE}${track.audio.src}`;
  if (!a.src.endsWith(track.audio.src)) {
    a.src = src;
    await new Promise((r) => {
      if (a.readyState >= 1) r();
      else a.addEventListener('loadedmetadata', r, { once: true });
    });
  }
  a.playbackRate = replay.speed;
  const base = recordedAt(track);
  if (base != null) installClock(() => base + replay.now * 1000);
  ctx.presentation.init({ slides: slidesFor(track) });
  loadCaptions(track);
}

// ---- the controls ------------------------------------------------------------------

/**
 * Bring the demo to how it was at `t` and continue from there. The chapter's
 * recorded state, then the chapter's events up to `t`, applied fast.
 */
export async function seek(t, { play: playAfter = replay.playing } = {}) {
  const track = currentTrack();
  if (!track || !ctx) return;
  seekToken += 1;
  const token = seekToken;
  audio?.pause();
  replay.playing = false;
  replay.waiting = false;
  replay.diverged = false;
  replay.cursor.visible = false;
  const target = Math.max(0, Math.min(track.duration, t));
  const ci = Math.max(0, chapterAt(track, target));
  const chapter = track.chapters[ci];
  replay.now = chapter.start;
  applyState(chapter.state);
  resetViews();
  await nextTick();
  if (!ctx.presentation.active.value) ctx.presentation.start(chapter.slideIndex, { keys: false });
  await ctx.presentation.goTo(chapter.slideIndex);
  await sleep(250);
  const events = track.events ?? [];
  // The chapter's snapshot was taken when its slide came up, so whatever comes
  // before that slide event in the list (actions moved out of a cut to the
  // same moment) is already in it. Replay from just after the slide event.
  const slideAt = events.findIndex((e) => e.type === 'slide' && e.index === chapter.slideIndex && e.t >= chapter.start - 0.001);
  let i = slideAt >= 0 ? slideAt + 1 : events.findIndex((e) => e.t >= chapter.start);
  if (i < 0) i = events.length;
  busyOwner = token;
  try {
    for (; i < events.length && events[i].t <= target; i += 1) {
      if (token !== seekToken || !replay.active) return;
      const e = events[i];
      replay.now = e.t;
      await apply(e, { fast: true, token });
    }
  } finally {
    if (busyOwner === token) busyOwner = null;
  }
  if (token !== seekToken) return;
  next = i;
  leadFor = -1;
  replay.now = target;
  if (audio) audio.currentTime = target;
  if (playAfter) play();
}

export async function play() {
  if (!audio) return;
  if (replay.diverged) {
    await seek(replay.now, { play: true });
    return;
  }
  replay.playing = true;
  // During a wait the voice stays where it is; the wait resumes it.
  if (!replay.waiting) {
    await audio.play().catch(() => {
      replay.playing = false;
    });
  }
  if (!raf) raf = requestAnimationFrame(tick);
}

export function pause() {
  replay.playing = false;
  audio?.pause();
  savePosition();
}

export function togglePlay() {
  return replay.playing ? pause() : play();
}

export function setSpeed(s) {
  replay.speed = s;
  if (audio) audio.playbackRate = s;
}

export function openFaq(entry) {
  if (!entry || !timeline) return;
  if (!replay.faq) mainPosition = replay.now;
  replay.faq = entry;
  loadTrack(entry).then(() => seek(0, { play: true }));
}

export function backToMain() {
  if (!replay.faq) return;
  replay.faq = null;
  loadTrack(timeline.main).then(() => seek(mainPosition, { play: true }));
}

function savePosition() {
  if (replay.faq) return;
  try {
    window.localStorage.setItem(POSITION_KEY, String(replay.now));
  } catch {
    /* no storage: start at the beginning next time */
  }
}

export function savedPosition() {
  try {
    const n = Number(window.localStorage.getItem(POSITION_KEY));
    return Number.isFinite(n) && n > 0 && n < (timeline?.main?.duration ?? 0) - 5 ? n : 0;
  } catch {
    return 0;
  }
}

// ---- the viewer taking over -------------------------------------------------------

/** Keys the player answers to; see ReplayOverlay.vue. */
export const PLAYER_KEYS = new Set([' ', 'ArrowLeft', 'ArrowRight', 'Escape']);

function fromViewer(e) {
  if (!e.isTrusted || !replay.active) return false;
  // The player's own controls and the deck are not the demo.
  return !(e.composedPath?.() ?? []).some((n) => n?.matches?.('.deck, .wt-chrome'));
}

function onViewerAct(e) {
  if (!fromViewer(e)) return;
  // A key is the viewer taking over only when it goes into the demo's own
  // fields. While it plays, Space, the arrows and Escape are the player's,
  // also when the replay itself left focus in a field it typed into.
  if (e.type === 'keydown') {
    if (replay.playing && PLAYER_KEYS.has(e.key)) return;
    if (!(e.composedPath?.() ?? []).some((n) => n?.matches?.('input, textarea, select, [contenteditable]'))) return;
  }
  if (replay.playing) pause();
  replay.diverged = true;
  replay.cursor.visible = false;
}

// ---- starting and stopping ---------------------------------------------------------

/**
 * Take over the demo for the walkthrough. `context` is the router, the
 * store (`useDemo()`) and the deck (`usePresentation()`).
 */
export async function startReplay(data, context, { at = 0, faqId = null } = {}) {
  timeline = data;
  ctx = context;
  // A live presentation that was still running would keep its arrow keys and
  // move the deck under the replay.
  ctx.presentation.stop();
  backup = clone(ctx.demo.state);
  setPersistence(false);
  // The recording is Dutch; its slides and the controls it clicks are found
  // by their Dutch text. The URL moves along with the replay anyway.
  previousLocale = currentLocale();
  adoptLocale(SOURCE_LOCALE);
  await nextTick();
  replay.active = true;
  replay.faq = null;
  document.addEventListener('pointerdown', onViewerAct, true);
  document.addEventListener('keydown', onViewerAct, true);
  const faq = faqId ? timeline.faq?.find((f) => f.id === faqId) : null;
  if (faq) mainPosition = at;
  replay.faq = faq ?? null;
  await loadTrack(currentTrack());
  ctx.presentation.start(currentTrack().chapters[0].slideIndex, { keys: false });
  await seek(faq ? 0 : at, { play: false });
}

export function stopReplay() {
  if (!replay.active) return;
  pause();
  seekToken += 1;
  cancelAnimationFrame(raf);
  raf = 0;
  document.removeEventListener('pointerdown', onViewerAct, true);
  document.removeEventListener('keydown', onViewerAct, true);
  uninstallClock();
  replay.active = false;
  replay.faq = null;
  replay.cursor.visible = false;
  replay.cues = [];
  if (audio) {
    audio.removeAttribute('src');
    audio.load();
  }
  const { demo, presentation } = ctx;
  presentation.stop();
  if (backup) Object.assign(demo.state, backup);
  setPersistence(true);
  demo.reregister();
  presentation.init({ slides: demo.corpus.value?.config?.slides ?? [] });
  if (previousLocale && previousLocale !== currentLocale()) {
    adoptLocale(previousLocale);
    // The URL decides the language on the next navigation; put the page in
    // the viewer's language now, so it does not flip back.
    const here = ctx.router.currentRoute.value;
    const page = here.meta?.page;
    if (page) ctx.router.replace({ name: localeRouteName(page, previousLocale), params: here.params }).catch(() => {});
  }
  resetViews();
  backup = null;
}

/** For the webcam bubble, which follows the voice. */
export function audioTime() {
  return audio?.currentTime ?? replay.now;
}

// A handle for scripted checks against the dev server (the Playwright runs
// that verify a recording); not in the production bundle.
if (import.meta.env.DEV && typeof window !== 'undefined') {
  window.__rrReplay = { replay, play, pause, seek, openFaq, backToMain, deckIndex: () => ctx?.presentation?.index?.value };
}
