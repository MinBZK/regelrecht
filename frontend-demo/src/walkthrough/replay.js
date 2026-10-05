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
import { adoptLocale, currentLocale, setViewerLocale, t } from '../i18n/index.js';
import { setPersistence } from '../store/demoStore.js';
import { localeRouteName, pageForConfigPath } from '../router.js';
import { click, setChecked, setValue, key as pressKey, scrollTo } from './actions.js';
import { installClock, uninstallClock } from './clock.js';
import { resolve, scrollAnchor, scrollTarget } from './locator.js';
import { graphView, showView } from './graphBridge.js';
import { CAPTIONS_BASE, MEDIA_BASE, chapterAt, parseVtt, pauseDue, pickClip } from './timeline.js';
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
  /** Actions whose element was not found: what a recording needs redone. */
  misses: [],
  /** Actions found only by a looser match: worth a look after a demo change. */
  loose: [],
  /** The media did not load: the page says so instead of spinning. */
  failed: false,
  /**
   * Why the player waits for the viewer, with a hint at the play button:
   * 'pause' at a moment the presenter invites them to look around, 'wait'
   * when they took over themselves. Null while it plays.
   */
  invite: null,
  /** The pause's own hint, from walkthrough.yaml; else the standard text. */
  inviteHint: null,
});

let timeline = null;
let ctx = null; // { router, demo, presentation }
let audio = null;
let next = 0; // index of the next event to apply
/** The seek token of whoever is applying events now (pump or seek), or null. */
let busyOwner = null;
/** The seek token of a seek in progress, or null. */
let seeking = null;
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
async function find(target, { timeout = 4000, fast = false, token = seekToken, event = null } = {}) {
  const started = performance.now();
  const limit = fast ? 1500 : timeout;
  let paused = false;
  for (;;) {
    if (!current(token)) {
      if (paused) replay.waiting = false;
      return null;
    }
    const waited = performance.now() - started;
    const exact = resolve(target);
    const el = exact ?? (waited > limit / 2 ? resolve(target, { loose: true }) : null);
    if (el) {
      if (!exact && event) noteLoose(event);
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

/**
 * Remember an action whose element was not found. The replay goes on (the
 * route still gets the viewer to the right tab), but this is the list of
 * what to record again after a demo change; the verify run reports it.
 */
function noteMiss(e) {
  const last = e.target?.at?.(-1) ?? {};
  replay.misses.push({ t: e.t, type: e.type, what: last.text ?? last['@aria-label'] ?? last.textContent ?? last.tag ?? '?' });
  if (import.meta.env.DEV) console.warn('[walkthrough] niet gevonden:', e.type, last);
}

/**
 * Remember an action that found its element only with the numbers in its
 * description masked ("Wetten, 79" became "Wetten, 81"). Usually harmless, a
 * count that moved; but it can also be a different element that now carries
 * those words, and then the recording means something else than it did.
 * The verify run lists these as warnings, for a person to look at.
 */
function noteLoose(e) {
  const last = e.target?.at?.(-1) ?? {};
  replay.loose.push({ t: e.t, type: e.type, what: last.text ?? last['@aria-label'] ?? last.textContent ?? last.tag ?? '?' });
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
      const el = await find(e.target, { ...opts, event: e });
      if (!el && current(token)) noteMiss(e);
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
      const el = await find(e.target, { ...opts, event: e });
      if (!el && current(token)) noteMiss(e);
      if (!el || !current(token)) return;
      if (!fast) showCursorAt(pointIn(el, 0.15, 0.5));
      setValue(el, e.value ?? '');
      return;
    }
    case 'change': {
      const el = await find(e.target, { ...opts, event: e });
      if (!el && current(token)) noteMiss(e);
      if (!el || !current(token)) return;
      if (typeof e.checked === 'boolean') setChecked(el, e.checked);
      else setValue(el, e.value ?? '', { change: true });
      return;
    }
    case 'key': {
      const el = e.target ? await find(e.target, { ...opts, timeout: 1000, event: e }) : null;
      if (!current(token)) return;
      if (el) pressKey(el, e.key);
      else document.dispatchEvent(new KeyboardEvent('keydown', { key: e.key, bubbles: true }));
      return;
    }
    case 'viewport': {
      // The graph may still be mounting after the tab opened; wait briefly.
      const deadline = performance.now() + (fast ? 800 : 1500);
      while (!graphView.api && performance.now() < deadline) {
        await sleep(60);
        if (!current(token)) return;
      }
      // A short glide between the samples (ten a second) looks like the
      // presenter's own drag; a jump just lands.
      if (current(token)) showView(e, fast ? 0 : 140);
      return;
    }
    case 'scroll': {
      const el = e.target ? await find(e.target, { ...opts, timeout: 1000, event: e }) : document.scrollingElement;
      if (!el || !current(token)) return;
      scrollTo(el, scrollTarget(el, e), e.left ?? 0);
      // For `walkthrough anchors`: what an older take lacks, measured in the
      // layout it was recorded in. Dev only.
      if (import.meta.env.DEV) window.__rrReplay?.afterScroll?.(e, el);
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

let lastTick = 0;
function tick() {
  if (audio && seeking === null) {
    const prev = lastTick;
    replay.now = audio.currentTime;
    lastTick = replay.now;
    // A moment the presenter hands the demo to the viewer: stop there.
    const due = replay.playing && !replay.waiting ? pauseDue(currentTrack()?.pauses, prev, replay.now) : null;
    if (due) {
      audio.pause();
      audio.currentTime = due.t;
      replay.now = due.t;
      lastTick = due.t;
      replay.playing = false;
      replay.invite = 'pause';
      replay.inviteHint = due.hint ?? null;
      savePosition();
    }
  }
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
    // A missing or broken file must end the wait: without an error path the
    // page shows its spinner forever.
    await new Promise((resolve, reject) => {
      if (a.readyState >= 1) return resolve();
      const timer = setTimeout(() => done(new Error(`timeout: ${src}`)), MEDIA_TIMEOUT);
      const done = (err) => {
        clearTimeout(timer);
        a.removeEventListener('loadedmetadata', ok);
        a.removeEventListener('error', fail);
        if (err) reject(err);
        else resolve();
      };
      const ok = () => done();
      const fail = () => done(new Error(`media: ${src}`));
      a.addEventListener('loadedmetadata', ok);
      a.addEventListener('error', fail);
    });
  }
  a.playbackRate = replay.speed;
  const base = recordedAt(track);
  if (base != null) installClock(replayClock(base));
  ctx.presentation.init({ slides: slidesFor(track) });
  loadCaptions(track);
}

const MEDIA_TIMEOUT = 20000;

/**
 * The time the demo sees during the replay: the recording's own date, moving
 * with the voice. While the voice stands still (paused, waiting, the viewer
 * clicking along) the clock keeps running on wall time, so code that stamps
 * ids or measures durations with `Date.now()` does not see time stop.
 */
function replayClock(base) {
  let extra = 0;
  let lastReal = performance.now();
  return () => {
    const real = performance.now();
    if (!replay.playing || replay.waiting || replay.diverged) extra += real - lastReal;
    lastReal = real;
    return base + replay.now * 1000 + extra;
  };
}

/** A token check for the steps after an await: a jump, a stop, the viewer leaving. */
function still(token) {
  return token === seekToken && replay.active;
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
  // From here the events are this seek's: a pump or a play in the meantime
  // would apply the old position's events onto the restored chapter.
  busyOwner = token;
  seeking = token;
  cancelAnimationFrame(raf);
  raf = 0;
  audio?.pause();
  // Play or pause pressed during the seek only changes what happens after it.
  replay.playing = playAfter;
  replay.invite = null;
  stopBumper();
  replay.waiting = false;
  replay.diverged = false;
  replay.cursor.visible = false;
  let i = 0;
  const target = Math.max(0, Math.min(track.duration, t));
  try {
    const ci = Math.max(0, chapterAt(track, target));
    const chapter = track.chapters[ci];
    replay.now = chapter.start;
    applyState(chapter.state);
    resetViews();
    await nextTick();
    if (!still(token)) return;
    if (!ctx.presentation.active.value) ctx.presentation.start(chapter.slideIndex, { keys: false });
    await ctx.presentation.goTo(chapter.slideIndex);
    await sleep(250);
    if (!still(token)) return;
    const events = track.events ?? [];
    // The chapter's snapshot was taken when its slide came up, so whatever comes
    // before that slide event in the list (actions moved out of a cut to the
    // same moment) is already in it. Replay from just after the slide event.
    const slideAt = events.findIndex((e) => e.type === 'slide' && e.index === chapter.slideIndex && e.t >= chapter.start - 0.001);
    i = slideAt >= 0 ? slideAt + 1 : events.findIndex((e) => e.t >= chapter.start);
    if (i < 0) i = events.length;
    for (; i < events.length && events[i].t <= target; i += 1) {
      if (!still(token)) return;
      const e = events[i];
      replay.now = e.t;
      await apply(e, { fast: true, token });
    }
  } finally {
    if (busyOwner === token) busyOwner = null;
    if (seeking === token) seeking = null;
  }
  if (!still(token)) return;
  next = i;
  leadFor = -1;
  replay.now = target;
  lastTick = target;
  if (audio) audio.currentTime = target;
  if (replay.playing) play();
}

export async function play() {
  // Still loading: the start of the replay plays when it is ready.
  if (!audio) {
    if (replay.active) replay.playing = true;
    return;
  }
  // A seek in progress plays when it is done.
  if (seeking !== null) {
    replay.playing = true;
    return;
  }
  if (replay.diverged) {
    // "Oké, we gaan verder" while the demo is put back the way it was.
    const said = sayBumper('resume');
    await seek(replay.now, { play: false });
    const token = seekToken;
    replay.playing = true;
    await said;
    // Paused, jumped or left during the line: that wins.
    if (token !== seekToken || !replay.active || !replay.playing || replay.diverged) return;
    replay.playing = false;
    await play();
    return;
  }
  replay.invite = null;
  replay.inviteHint = null;
  lastTick = replay.now;
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
  stopBumper();
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
  switchTo(entry, 0);
}

export function backToMain() {
  if (!replay.faq) return;
  replay.faq = null;
  switchTo(timeline.main, mainPosition);
}

/** Load another track and play it from `at`, unless something else happened first. */
function switchTo(track, at) {
  // Whatever was jumping or playing stops here.
  seekToken += 1;
  const token = seekToken;
  // Not pause(): that would store the question's position as the main line's.
  replay.playing = false;
  audio?.pause();
  loadTrack(track).then(
    () => still(token) && seek(at, { play: true }),
    () => still(token) && fail(),
  );
}

/** The media did not load: leave the replay and let the page say so. */
function fail() {
  stopReplay();
  replay.failed = true;
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
  takeOver({ say: replay.playing });
}

/**
 * The viewer takes the demo over (clicked into it, or "try it yourself"):
 * pause, and wait for them with a hint at the play button. `say` adds a line
 * in the presenter's voice ("Ga je gang, kijk maar even rond, ik wacht"),
 * when the walkthrough has one; only when it was playing, not when it had
 * already stopped at a pause that said as much.
 */
export function takeOver({ say = false } = {}) {
  if (replay.playing) pause();
  replay.diverged = true;
  replay.cursor.visible = false;
  if (!replay.invite) {
    replay.invite = 'wait';
    replay.inviteHint = null;
  }
  if (say) sayBumper('wait');
}

// ---- the presenter's short lines ---------------------------------------------------

let bumperAudio = null;
let bumperDone = null;

/**
 * Play one of the walkthrough's short lines of `kind` ('wait', 'resume'),
 * picked at random; resolves when it has been said, or at once when there
 * is none. Its own audio element: it must not move the main clock.
 */
function sayBumper(kind) {
  stopBumper();
  const clip = pickClip(timeline?.bumpers?.[kind]);
  if (!clip) return Promise.resolve();
  bumperAudio = bumperAudio ?? new Audio();
  bumperAudio.src = `${MEDIA_BASE}${clip.src}`;
  bumperAudio.muted = audio?.muted ?? false;
  bumperAudio.playbackRate = replay.speed;
  return new Promise((resolve) => {
    let timer = 0;
    const done = () => {
      clearTimeout(timer);
      bumperAudio?.removeEventListener('ended', done);
      bumperAudio?.removeEventListener('error', done);
      if (bumperDone === done) bumperDone = null;
      resolve();
    };
    bumperDone = done;
    bumperAudio.addEventListener('ended', done);
    bumperAudio.addEventListener('error', done);
    // A line that never ends (a stalled download) does not hold the replay.
    timer = setTimeout(done, ((clip.duration ?? 3) / replay.speed + 2) * 1000);
    bumperAudio.play().catch(done);
  });
}

function stopBumper() {
  bumperAudio?.pause();
  bumperDone?.();
}

// ---- starting and stopping ---------------------------------------------------------

/**
 * Take over the demo for the walkthrough. `context` is the router, the
 * store (`useDemo()`) and the deck (`usePresentation()`).
 */
export async function startReplay(data, context, { at = 0, faqId = null } = {}) {
  // A kept-alive page is mounted and activated in the same tick, and both
  // start the walkthrough; the second would take the first one's Dutch for
  // the viewer's language and run a second replay over it.
  if (replay.active || starting) return;
  starting = true;
  try {
    await begin(data, context, { at, faqId });
  } finally {
    starting = false;
  }
}

let starting = false;

async function begin(data, context, { at, faqId }) {
  timeline = data;
  ctx = context;
  // A live presentation that was still running would keep its arrow keys and
  // move the deck under the replay.
  ctx.presentation.stop();
  backup = clone(ctx.demo.state);
  setPersistence(false);
  // The recording is Dutch; its slides and the controls it clicks are found
  // by their Dutch text, and the voice names them. The demo runs in Dutch for
  // the length of the replay; the player's own controls stay in the viewer's
  // language.
  previousLocale = currentLocale();
  setViewerLocale(previousLocale);
  adoptLocale(SOURCE_LOCALE);
  await nextTick();
  replay.active = true;
  const loading = seekToken;
  replay.faq = null;
  document.addEventListener('pointerdown', onViewerAct, true);
  document.addEventListener('keydown', onViewerAct, true);
  const faq = faqId ? timeline.faq?.find((f) => f.id === faqId) : null;
  if (faq) mainPosition = at;
  replay.faq = faq ?? null;
  try {
    await loadTrack(currentTrack());
  } catch {
    fail();
    return;
  }
  // A jump made while the track was loading (a chapter picked, the arrows)
  // already put the demo where the viewer wants it.
  if (seekToken !== loading) return;
  ctx.presentation.start(currentTrack().chapters[0].slideIndex, { keys: false });
  // Play pressed while loading plays from here, rather than being undone.
  await seek(faq ? 0 : at, { play: replay.playing });
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
  replay.invite = null;
  replay.cursor.visible = false;
  stopBumper();
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
  setViewerLocale(null);
  backup = null;
}

/** For the webcam bubble, which follows the voice. */
export function audioTime() {
  return audio?.currentTime ?? replay.now;
}

// A handle for scripted checks against the dev server (the Playwright runs
// that verify a recording); not in the production bundle.
if (import.meta.env.DEV && typeof window !== 'undefined') {
  window.__rrReplay = { replay, play, pause, seek, openFaq, backToMain, scrollAnchor, deckIndex: () => ctx?.presentation?.index?.value };
}
