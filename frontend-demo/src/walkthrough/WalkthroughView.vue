<script setup>
/**
 * The recorded walkthrough: the presenter's voice and the demo as video, with
 * the slides as real text in the deck next to it.
 *
 * The app is a recording, not a replay. That is a choice: replaying clicks in
 * the live app would make every amount depend on the day it is watched, every
 * case id on chance, and every tour on the exact layout of the viewer's
 * window. A video shows what was said, on the day it was recorded. What
 * stays live is the text: the slides render from the timeline, so a sentence
 * can be corrected without a retake. "Probeer het zelf" opens the live demo at
 * the same spot for anyone who wants to click.
 *
 * The app video is the clock. The webcam bubble follows it; the slides, the
 * captions, the click ripples and the questions are read off its current time.
 */
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import PresentationDeck from '../presentation/PresentationDeck.vue';
import { slideTarget, usePresentation } from '../presentation/usePresentation.js';
import { useDemo } from '../store/demoStore.js';
import { DEFAULT_LOCALE, useI18n } from '../i18n/index.js';
import { intlLocale } from '../data/format.js';
import { localeRouteName } from '../router.js';
import { useWalkthrough } from './useWalkthrough.js';
import {
  CAPTIONS_BASE,
  MEDIA_BASE,
  chapterAt,
  cueAt,
  fitRect,
  followerCorrection,
  formatTime,
  nextChapterStart,
  offeredFaq,
  parseVtt,
  previousChapterStart,
  profileAt,
  ripplesAt,
} from './timeline.js';

const POSITION_KEY = 'rr-walkthrough-position-v1';
const SPEEDS = [1, 1.25, 1.5];
/** "1,25×" in Dutch, "1.25×" in English: the decimal sign is the locale's. */
function speedText(s) {
  return `${new Intl.NumberFormat(intlLocale()).format(s)}×`;
}

const { t, locale } = useI18n();
const route = useRoute();
const router = useRouter();
const demo = useDemo();
const presentation = usePresentation();
const { timeline, ready } = useWalkthrough();

const loaded = ref(false);
/**
 * Still mode (`?still=<seconds>&track=<main|faq id>`): the deck as it stands at
 * that moment, without video, bubble, captions or controls. The MP4 export
 * (script/walkthrough/walkthrough/export.py) screenshots it once per chapter
 * and lays the video over it.
 */
const still = route.query.still != null ? Number(route.query.still) : null;
const stillReady = ref(false);
const video = ref(null);
const cam = ref(null);

const now = ref(0);
const playing = ref(false);
const speed = ref(1);
const captionsOn = ref(true);
const camOn = ref(true);
/** The FAQ entry being played, or null for the main line. */
const faq = shallowRef(null);
/** Where the main line was when a question was opened. */
let mainPosition = 0;
const cues = shallowRef([]);
/** The empty spot in the rail where the bubble goes; the video is laid over it. */
const camSlot = ref(null);
const camStyle = ref({});
const stageSize = ref({ width: 0, height: 0 });

const track = computed(() => faq.value ?? timeline.value?.main ?? null);
const chapterIndex = computed(() => (track.value ? chapterAt(track.value, now.value) : -1));
const chapter = computed(() => track.value?.chapters?.[chapterIndex.value] ?? null);

/**
 * The slide the deck shows. In the main line that is the recorded slide; in an
 * answer it is the question itself, laid out the way the recording was (a rail
 * next to the demo when the presenter clicked in it, a full slide when not).
 */
const slide = computed(() => {
  const s = chapter.value?.slide ?? null;
  if (!faq.value) return s;
  return {
    kind: s?.route ? undefined : 'section',
    route: s?.route,
    overline: t('walkthrough.faq.overline'),
    title: faq.value.question,
  };
});
const isFull = computed(() => !slide.value?.route);
const offered = computed(() => (faq.value ? [] : offeredFaq(timeline.value, now.value)));
const cue = computed(() => (captionsOn.value ? cueAt(cues.value, now.value) : null));
const progress = computed(() => (track.value?.duration ? Math.min(100, (now.value / track.value.duration) * 100) : 0));
const timeText = computed(() => `${formatTime(now.value)} / ${formatTime(track.value?.duration ?? 0)}`);

const ripples = computed(() => {
  if (isFull.value || !track.value?.video) return [];
  const box = fitRect(stageSize.value.width, stageSize.value.height, track.value.video.width, track.value.video.height);
  return ripplesAt(track.value.clicks, now.value).map((c) => ({
    key: `${c.t}-${c.x}-${c.y}`,
    left: `${box.left + c.x * box.width}px`,
    top: `${box.top + c.y * box.height}px`,
  }));
});

function mediaUrl(src) {
  return src ? `${MEDIA_BASE}${src}` : '';
}

async function loadCaptions() {
  const file = track.value?.captions?.nl;
  cues.value = [];
  if (!file) return;
  try {
    const res = await fetch(`${CAPTIONS_BASE}${file}`);
    if (res.ok) cues.value = parseVtt(await res.text());
  } catch {
    /* no captions is not a reason to stop the walkthrough */
  }
}

// ---- the clock -------------------------------------------------------------

let raf = 0;
let lastSync = 0;
function tick() {
  const v = video.value;
  if (v) {
    now.value = v.currentTime;
    const c = cam.value;
    if (c && camOn.value && track.value?.cam && performance.now() - lastSync > 250) {
      lastSync = performance.now();
      const fix = followerCorrection(v.currentTime, c.currentTime, speed.value);
      if (fix.seek != null) c.currentTime = fix.seek;
      c.playbackRate = fix.rate;
    }
  }
  raf = playing.value ? requestAnimationFrame(tick) : 0;
}

function onPlay() {
  playing.value = true;
  cam.value?.play?.().catch(() => {});
  if (!raf) raf = requestAnimationFrame(tick);
}
function onPause() {
  playing.value = false;
  cam.value?.pause?.();
  savePosition();
  tick();
}
function onSeeked() {
  now.value = video.value?.currentTime ?? 0;
  if (cam.value) cam.value.currentTime = now.value;
}
function onEnded() {
  playing.value = false;
  if (faq.value) backToMain();
}

function togglePlay() {
  const v = video.value;
  if (!v) return;
  if (v.paused) v.play().catch(() => {});
  else v.pause();
}

function seek(to) {
  const v = video.value;
  if (!v || !track.value) return;
  v.currentTime = Math.max(0, Math.min(track.value.duration, to));
  now.value = v.currentTime;
}

function skip(delta) {
  seek(now.value + delta);
}
function prevChapter() {
  seek(previousChapterStart(track.value, now.value));
}
function nextChapter() {
  const to = nextChapterStart(track.value, now.value);
  if (to != null) seek(to);
}
function goToChapter(i) {
  const c = track.value?.chapters?.[i];
  if (c) seek(c.start);
}

function setSpeed(s) {
  speed.value = s;
  if (video.value) video.value.playbackRate = s;
  if (cam.value) cam.value.playbackRate = s;
}

// ---- questions ---------------------------------------------------------------

const backButton = ref(null);
const playButton = ref(null);

function openFaq(entry, { autoplay = true } = {}) {
  if (!entry) return;
  // Before its metadata is in, the video reports 0: keep the position that
  // was restored from storage instead (a deep link to a question).
  if (!faq.value && (video.value?.readyState ?? 0) >= 1) mainPosition = video.value.currentTime;
  faq.value = entry;
  now.value = 0;
  if (route.params.faqId !== entry.id) router.replace({ name: localeRouteName('rondleiding', locale.value), params: { faqId: entry.id } });
  loadTrack(0, autoplay);
  // The question that had focus is gone from the list now; hand focus to the
  // way back, so a keyboard user is not dropped on the page.
  nextTick(() => backButton.value?.focus?.());
}

function backToMain() {
  faq.value = null;
  if (route.params.faqId) router.replace({ name: localeRouteName('rondleiding', locale.value) });
  loadTrack(mainPosition, true);
  nextTick(() => playButton.value?.focus?.());
}

/** Point the media at the current track and continue at `at`. */
async function loadTrack(at, autoplay) {
  await nextTick();
  const v = video.value;
  if (!v) return;
  const onReady = () => {
    v.currentTime = at;
    if (cam.value) cam.value.currentTime = at;
    setSpeed(speed.value);
    now.value = at;
    if (autoplay) v.play().catch(() => {});
  };
  if (v.readyState >= 1) onReady();
  else v.addEventListener('loadedmetadata', onReady, { once: true });
  loadCaptions();
}

// ---- trying it yourself ------------------------------------------------------

const tryTarget = computed(() => (slide.value?.route ? slideTarget(slide.value.route) : null));

function tryIt() {
  if (!tryTarget.value) return;
  video.value?.pause();
  savePosition();
  const profile = profileAt(track.value, chapterIndex.value);
  if (profile && demo.profileKey.value !== profile) demo.setProfile(profile);
  router.push(tryTarget.value);
}

// ---- position and keys -------------------------------------------------------

function savePosition() {
  if (faq.value) return;
  try {
    window.localStorage.setItem(POSITION_KEY, String(video.value?.currentTime ?? now.value));
  } catch {
    /* storage unavailable: the walkthrough then starts at the beginning */
  }
}
function savedPosition() {
  try {
    const n = Number(window.localStorage.getItem(POSITION_KEY));
    return Number.isFinite(n) && n > 0 && n < (timeline.value?.main?.duration ?? 0) - 5 ? n : 0;
  } catch {
    return 0;
  }
}

function pathHas(e, selector) {
  return (e.composedPath?.() ?? []).some((n) => n?.matches?.(selector));
}

/** Whether the transcript panel is open; the player's keys then stay off. */
const transcriptOpen = ref(false);

function onKey(e) {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  if (transcriptOpen.value) return;
  // A menu or a field handles its own keys, arrows included.
  if (pathHas(e, 'input, textarea, select, nldd-menu, nldd-menu-item, [role="menu"]')) return;
  // Spatie op een knop met focus drukt die knop in; die mag niet tegelijk
  // pauzeren, anders heft de ene handeling de andere op. De pijltjes doet een
  // knop(penbalk) niets mee, dus die blijven hoofdstukken springen.
  if (e.key === ' ' && pathHas(e, 'button, [role="button"], a[href], nldd-button, nldd-icon-button')) return;
  switch (e.key) {
    case ' ':
    case 'k':
      e.preventDefault();
      togglePlay();
      break;
    case 'ArrowLeft':
      e.preventDefault();
      prevChapter();
      break;
    case 'ArrowRight':
      e.preventDefault();
      nextChapter();
      break;
    case 'j':
      skip(-10);
      break;
    case 'l':
      skip(10);
      break;
    case 'c':
      captionsOn.value = !captionsOn.value;
      break;
    case 'Escape':
      if (faq.value) backToMain();
      break;
    default:
  }
}

let resizeObserver = null;
function measure() {
  const el = video.value;
  if (el) stageSize.value = { width: el.clientWidth, height: el.clientHeight };
  placeCam();
}

/**
 * Lay the bubble over its spot in the rail. The spot is part of the deck's
 * flow, so the bubble never covers the controls or a question, whatever the
 * slide's length; the video itself stays one element, fixed, so moving it
 * does not restart it. On a slide that covers the screen there is no spot and
 * the stylesheet puts the bubble bottom right, larger.
 */
function placeCam() {
  const r = camSlot.value?.getBoundingClientRect?.();
  camStyle.value = r && r.width > 0 ? { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px`, bottom: 'auto' } : {};
}
watch([chapterIndex, isFull, () => offered.value.length, camOn, faq], () => nextTick(placeCam));

onMounted(async () => {
  // The live deck and its keys would fight this page for the arrows.
  if (presentation.active.value) presentation.stop();
  await ready;
  loaded.value = true;
  if (!timeline.value) return;
  if (still != null) {
    const id = route.query.track;
    faq.value = id && id !== 'main' ? timeline.value.faq?.find((f) => f.id === id) ?? null : null;
    now.value = still;
    await nextTick();
    await document.fonts?.ready;
    stillReady.value = true;
    return;
  }
  await nextTick();
  window.addEventListener('keydown', onKey);
  window.addEventListener('resize', measure);
  resizeObserver = new ResizeObserver(measure);
  if (video.value) resizeObserver.observe(video.value);
  measure();
  mainPosition = savedPosition();
  const deep = timeline.value.faq?.find((f) => f.id === route.params.faqId);
  if (deep) openFaq(deep, { autoplay: false });
  else loadTrack(mainPosition, false);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKey);
  window.removeEventListener('resize', measure);
  resizeObserver?.disconnect();
  cancelAnimationFrame(raf);
  savePosition();
});

watch(
  () => route.params.faqId,
  (id) => {
    if (!timeline.value) return;
    if (!id && faq.value) backToMain();
    else if (id && id !== faq.value?.id) openFaq(timeline.value.faq?.find((f) => f.id === id));
  },
);

// ---- the transcript ----------------------------------------------------------
// The spoken text as text, per chapter: for whoever cannot or does not want to
// listen, and for a screen reader, which the captions over the video do not
// serve. Each chapter can be played from its start.

const transcriptSheet = ref(null);
const transcript = computed(() =>
  (track.value?.chapters ?? []).map((c, i, all) => {
    const end = all[i + 1]?.start ?? Infinity;
    return {
      start: c.start,
      title: c.slide?.title ?? c.slide?.lines?.[0]?.replaceAll('**', '') ?? t('walkthrough.chapter', { n: i + 1 }),
      text: cues.value.filter((q) => q.start >= c.start && q.start < end).map((q) => q.text.replace(/\n/g, ' ')).join(' '),
    };
  }),
);
function openTranscript() {
  video.value?.pause();
  transcriptSheet.value?.show?.();
}
function playFrom(at) {
  transcriptSheet.value?.hide?.();
  seek(at);
  video.value?.play().catch(() => {});
}

const presenter = computed(() => [timeline.value?.presenter?.name, timeline.value?.presenter?.role].filter(Boolean).join(', '));
const homePath = computed(() => router.resolve({ name: localeRouteName('home', locale.value) }).path);
</script>

<template>
  <div v-if="loaded && timeline" class="walkthrough" :class="{ full: isFull, still: still != null }" :data-still-ready="stillReady || undefined">
    <PresentationDeck :slide="slide" :index="chapterIndex" :total="track.chapters.length" :presenter="presenter" content-lang="nl">
      <template #aside>
        <div v-if="(track.cam && camOn && !isFull) || (offered.length && still == null)" class="aside">
        <div v-if="track.cam && camOn && !isFull" ref="camSlot" class="cam-slot" aria-hidden="true"></div>
        <div v-if="offered.length && still == null" class="faq" lang="nl" role="region" :aria-label="t('walkthrough.faq.label')">
          <span class="faq-title">{{ t('walkthrough.faq.title') }}</span>
          <TransitionGroup name="faq" tag="div" class="faq-items">
            <nldd-button
              v-for="f in offered"
              :key="f.id"
              size="sm"
              variant="inherit-tinted"
              start-icon="question"
              horizontal-alignment="left"
              width="full"
              :text="f.question"
              @click="openFaq(f)"
            ></nldd-button>
          </TransitionGroup>
        </div>
        </div>
      </template>
      <template #footer>
        <div v-if="still != null"></div>
        <div v-else class="controls">
          <!-- The recording, the slides and the captions are Dutch in every
               interface language; say so, as the law pages do. -->
          <nldd-text v-if="locale !== DEFAULT_LOCALE" size="sm" color="inherit">{{ t('walkthrough.dutch_only') }}</nldd-text>
          <nldd-progress-bar size="sm" color="donkergeel" :value="progress" max="100" value-display="none" :accessible-label="t('walkthrough.progress_at', { time: timeText })"></nldd-progress-bar>
          <!-- Two rows and one contextual button. What a viewer reaches for
               (play, chapters) is a button; the rest (speed, captions,
               transcript, the bubble, leaving) sits under one menu. Ten
               seconds back and forth stay on j and l. -->
          <div class="control-row">
            <nldd-button-bar>
              <nldd-icon-button variant="inherit-tinted" icon="media-backward-end" :text="t('walkthrough.prev_chapter')" @click="prevChapter"></nldd-icon-button>
              <nldd-icon-button ref="playButton" variant="inherit-filled" :icon="playing ? 'pause' : 'play'" :text="playing ? t('walkthrough.pause') : t('walkthrough.play')" @click="togglePlay"></nldd-icon-button>
              <nldd-icon-button variant="inherit-tinted" icon="media-forward-end" :text="t('walkthrough.next_chapter')" @click="nextChapter"></nldd-icon-button>
            </nldd-button-bar>
            <span class="time" aria-hidden="true">{{ timeText }}</span>
            <nldd-button-bar>
              <nldd-button variant="inherit-tinted" size="sm" :text="t('walkthrough.chapters')" expandable popup-type="menu">
                <nldd-menu slot="popup" :accessible-label="t('walkthrough.chapters')">
                  <nldd-menu-item
                    v-for="(c, i) in track.chapters"
                    :key="i"
                    type="radio"
                    :text="c.slide?.title ?? c.slide?.lines?.[0]?.replaceAll('**', '') ?? t('walkthrough.chapter', { n: i + 1 })"
                    :details="formatTime(c.start)"
                    :selected="i === chapterIndex || undefined"
                    @select="goToChapter(i)"
                  ></nldd-menu-item>
                </nldd-menu>
              </nldd-button>
              <nldd-icon-button variant="inherit-tinted" icon="more" :text="t('walkthrough.more')" expandable popup-type="menu">
                <nldd-menu slot="popup" :accessible-label="t('walkthrough.more')">
                  <!-- `@select` on each item, not on the group: the same
                       reason as the toolbar menus in App.vue. -->
                  <nldd-menu-group :text="t('walkthrough.speed')">
                    <nldd-menu-item v-for="s in SPEEDS" :key="s" type="radio" :text="speedText(s)" :selected="s === speed || undefined" @select="setSpeed(s)"></nldd-menu-item>
                  </nldd-menu-group>
                  <nldd-menu-group :text="t('walkthrough.view')">
                    <nldd-menu-item type="checkbox" icon="message-rectangle-text" :text="t('walkthrough.captions')" :selected="captionsOn || undefined" @select="captionsOn = !captionsOn"></nldd-menu-item>
                    <nldd-menu-item v-if="track.cam" type="checkbox" icon="person-circle" :text="t('walkthrough.camera')" :selected="camOn || undefined" @select="camOn = !camOn"></nldd-menu-item>
                    <nldd-menu-item icon="text-document" :text="t('walkthrough.transcript.open')" @select="openTranscript"></nldd-menu-item>
                  </nldd-menu-group>
                  <nldd-menu-group>
                    <nldd-menu-item icon="home" :text="t('walkthrough.leave')" @select="router.push(homePath)"></nldd-menu-item>
                  </nldd-menu-group>
                </nldd-menu>
              </nldd-icon-button>
            </nldd-button-bar>
          </div>
          <!-- One way on from here: back from a question, or into the live
               demo at this spot. -->
          <nldd-button v-if="faq" ref="backButton" variant="inherit-filled" size="sm" start-icon="back" :text="t('walkthrough.faq.back')" @click="backToMain"></nldd-button>
          <nldd-button v-else-if="tryTarget" variant="inherit-tinted" size="sm" start-icon="hand" :text="t('walkthrough.try')" @click="tryIt"></nldd-button>
        </div>
      </template>
    </PresentationDeck>

    <main class="wt-stage" :style="{ '--video-ratio': `${track.video.width} / ${track.video.height}` }" :aria-label="t('walkthrough.label')">
      <video
        v-if="still == null"
        ref="video"
        class="app-video"
        :src="mediaUrl(track.video.src)"
        playsinline
        preload="auto"
        @play="onPlay"
        @pause="onPause"
        @seeked="onSeeked"
        @ended="onEnded"
        @loadedmetadata="measure"
        @click="togglePlay"
      ></video>
      <span v-for="r in ripples" :key="r.key" class="ripple" :style="{ left: r.left, top: r.top }" aria-hidden="true"></span>
      <!-- On a phone the caption goes under the picture, not over the slide
           text below it; the one outside the stage is for wider screens. -->
      <p v-if="cue" class="caption caption-in-stage" lang="nl" aria-hidden="true">{{ cue.text }}</p>
    </main>

    <video
      v-if="track.cam && still == null"
      v-show="camOn"
      ref="cam"
      class="cam"
      :style="camStyle"
      :src="mediaUrl(track.cam.src)"
      muted
      playsinline
      preload="auto"
      aria-hidden="true"
    ></video>

    <p v-if="cue" class="caption caption-over" lang="nl" aria-hidden="true">{{ cue.text }}</p>

    <nldd-sheet ref="transcriptSheet" placement="right" width="560px" :accessible-label="t('walkthrough.transcript.title')" @open="transcriptOpen = true" @close="transcriptOpen = false">
      <nldd-page>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="t('walkthrough.transcript.title')" :supporting-text="faq?.question" :dismiss-text="t('walkthrough.transcript.close')" @dismiss="transcriptSheet?.hide?.()"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="24" lang="nl">
          <nldd-container v-for="(c, i) in transcript" :key="i" padding="0" gap="8">
            <nldd-title size="5">
              <h2>{{ c.title }}</h2>
            </nldd-title>
            <nldd-rich-text v-if="c.text" spacing="tight">
              <p>{{ c.text }}</p>
            </nldd-rich-text>
            <nldd-button size="sm" variant="neutral-transparent" start-icon="play" :lang="locale" :text="t('walkthrough.transcript.play_from', { time: formatTime(c.start) })" @click="playFrom(c.start)"></nldd-button>
          </nldd-container>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </div>

  <nldd-page v-else-if="loaded">
    <nldd-simple-section width="720px">
      <nldd-title slot="header" size="2">
        <h1>{{ t('walkthrough.missing.title') }}</h1>
        <span slot="subtitle">{{ t('walkthrough.missing.body') }}</span>
      </nldd-title>
      <nldd-button variant="primary" start-icon="home" :text="t('walkthrough.leave')" @click="router.push(homePath)"></nldd-button>
    </nldd-simple-section>
  </nldd-page>
</template>

<style scoped>
/* Custom CSS, on purpose: the design system has no video player, no picture-
 * in-picture bubble and no caption overlay. The controls themselves are nldd
 * buttons, menus and a progress bar in the deck's footer slot. What is here is
 * placement: the video next to the rail, the bubble in the rail, the captions
 * over the video, and the ripple that shows where the presenter clicked. */
.walkthrough {
  --rail: 36vw;
  /* A video stage is dark in either theme; the palette's scales flip in dark
     mode, so pin the light side (as the deck does) and pick its darkest step. */
  color-scheme: light;
  min-height: 100vh;
  background: var(--primitives-color-coolgray-950);
}
.wt-stage {
  position: fixed;
  inset: 0 0 0 var(--rail);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--primitives-color-coolgray-950);
}
.app-video {
  width: 100%;
  height: 100%;
  object-fit: contain;
  display: block;
  cursor: pointer;
}
.ripple {
  position: absolute;
  width: 44px;
  height: 44px;
  margin: -22px 0 0 -22px;
  border-radius: 50%;
  border: 3px solid var(--primitives-color-donkergeel-200);
  pointer-events: none;
  animation: rr-ripple 0.7s ease-out forwards;
}
@keyframes rr-ripple {
  from {
    transform: scale(0.3);
    opacity: 1;
  }
  to {
    transform: scale(1.4);
    opacity: 0;
  }
}

/* The bubble: in the rail it is laid over `.cam-slot` (inline style from
 * placeCam); on a slide that covers the screen it is larger, bottom right.
 * Both are expressed in left/top/width/height, so the move between them
 * animates. */
.cam {
  position: fixed;
  z-index: 85;
  width: 240px;
  height: 240px;
  left: calc(100vw - 240px - 4rem);
  top: calc(100vh - 240px - 12rem);
  box-sizing: border-box;
  border-radius: 50%;
  object-fit: cover;
  box-shadow: 0 6px 24px rgb(0 0 0 / 0.35);
  border: 3px solid var(--primitives-color-coolgray-0);
  transition: left 0.5s cubic-bezier(0.22, 1, 0.36, 1), top 0.5s cubic-bezier(0.22, 1, 0.36, 1), width 0.5s, height 0.5s;
}
.aside {
  display: flex;
  align-items: flex-end;
  gap: 1.25rem;
  margin-top: 1rem;
}
.cam-slot {
  flex: 0 0 auto;
  width: 120px;
  height: 120px;
}

.caption {
  position: fixed;
  z-index: 86;
  left: calc(var(--rail) + 2rem);
  right: 2rem;
  bottom: 2rem;
  margin: 0 auto;
  width: fit-content;
  max-width: 60ch;
  padding: 0.4rem 0.8rem;
  border-radius: 6px;
  background: rgb(0 0 0 / 0.78);
  color: var(--primitives-color-coolgray-0);
  font-family: 'RijksSans', system-ui, sans-serif;
  font-size: clamp(1rem, 1.4vw, 1.4rem);
  line-height: 1.35;
  text-align: center;
  white-space: pre-line;
}
.full .caption {
  left: 2rem;
  bottom: 12rem;
}
.caption-in-stage {
  display: none;
}

.faq {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.faq-title {
  font-size: 0.85rem;
  font-weight: 600;
  opacity: 0.72;
}
.faq-items {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.faq-enter-active {
  transition: opacity 0.6s ease, transform 0.6s ease;
}
.faq-enter-from {
  opacity: 0;
  transform: translateY(6px);
}

.controls {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  padding-top: 1rem;
}
.controls > nldd-button {
  align-self: flex-start;
}
.control-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem 1rem;
}
.time {
  font-variant-numeric: tabular-nums;
  font-size: 0.9rem;
  opacity: 0.8;
}

/* A phone or a narrow window: the video on top, the slide and the controls
 * under it, in the page's own flow. */
@media (max-width: 1024px) {
  .walkthrough {
    --rail: 0px;
    display: flex;
    flex-direction: column;
  }
  .wt-stage {
    position: relative;
    inset: auto;
    aspect-ratio: var(--video-ratio, 16 / 10);
    order: 1;
  }
  /* On a phone the bubble sits in the corner of the video, small. */
  .cam,
  .full .cam {
    width: 88px !important;
    height: 88px !important;
    left: auto !important;
    right: 0.75rem;
    top: 0.75rem !important;
  }
  .cam-slot {
    display: none;
  }
  .caption-over {
    display: none;
  }
  .caption-in-stage {
    display: block;
    position: absolute;
    left: 0.5rem;
    right: 0.5rem;
    bottom: 0.5rem;
    font-size: 0.95rem;
  }
}
@media (prefers-reduced-motion: reduce) {
  .cam,
  .faq-enter-active {
    transition: none;
  }
  .ripple {
    animation-duration: 0.01s;
  }
}
</style>
