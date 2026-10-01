<script setup>
/**
 * What lies over the live demo while a walkthrough runs: the presenter in a
 * circle, the captions, the cursor that shows where the presenter clicks, and
 * the transcript panel. Also the player's keys.
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { useI18n } from '../i18n/index.js';
import { usePresentation } from '../presentation/usePresentation.js';
import { audioTime, backToMain, currentTrack, replay, seek, togglePlay } from './replay.js';
import { MEDIA_BASE, cueAt, followerCorrection, formatTime, nextChapterStart, previousChapterStart } from './timeline.js';
import { camSlot, transcript } from './chrome.js';

const { t, locale } = useI18n();
const p = usePresentation();

const track = computed(() => currentTrack());
const cue = computed(() => (replay.captionsOn ? cueAt(replay.cues, replay.now) : null));
const full = computed(() => p.isFull.value);

// ---- the bubble -------------------------------------------------------------------

const cam = ref(null);
const camStyle = ref({});

/** Lay the bubble over its spot in the rail; on a full slide the stylesheet places it. */
function placeCam() {
  const r = camSlot.value?.getBoundingClientRect?.();
  camStyle.value = !full.value && r && r.width > 0 ? { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` } : {};
}

let raf = 0;
let lastSync = 0;
function loop() {
  const c = cam.value;
  if (c && replay.camOn && track.value?.cam && performance.now() - lastSync > 250) {
    lastSync = performance.now();
    const fix = followerCorrection(audioTime(), c.currentTime, replay.speed);
    if (fix.seek != null) c.currentTime = fix.seek;
    c.playbackRate = fix.rate;
    if (replay.playing && !replay.waiting && c.paused) c.play().catch(() => {});
    if ((!replay.playing || replay.waiting) && !c.paused) c.pause();
  }
  placeCam();
  raf = requestAnimationFrame(loop);
}

// ---- the top layer ---------------------------------------------------------------------

const pointerLayer = ref(null);
function raise() {
  const el = pointerLayer.value;
  if (!el?.showPopover) return;
  try {
    if (el.matches(':popover-open')) el.hidePopover();
    el.showPopover();
  } catch {
    /* not supported: the layer stays in normal stacking */
  }
}
watch(() => [replay.cursor.x, replay.cursor.y, replay.ripples.length, cue.value?.text], raise);
onMounted(raise);

// ---- the transcript ------------------------------------------------------------------

const sheet = ref(null);
watch(
  () => transcript.open,
  (open) => (open ? sheet.value?.show?.() : sheet.value?.hide?.()),
);
const chapters = computed(() =>
  (track.value?.chapters ?? []).map((c, i, all) => {
    const end = all[i + 1]?.start ?? Infinity;
    return {
      start: c.start,
      title: c.slide?.title ?? c.slide?.lines?.[0]?.replaceAll('**', '') ?? t('walkthrough.chapter', { n: i + 1 }),
      text: replay.cues
        .filter((q) => q.start >= c.start && q.start < end)
        .map((q) => q.text.replace(/\n/g, ' '))
        .join(' '),
    };
  }),
);
function playFrom(at) {
  transcript.open = false;
  seek(at, { play: true });
}

// ---- keys ------------------------------------------------------------------------------

function pathHas(e, selector) {
  return (e.composedPath?.() ?? []).some((n) => n?.matches?.(selector));
}

function onKey(e) {
  if (!replay.active || e.metaKey || e.ctrlKey || e.altKey || transcript.open) return;
  // A field, a menu or the demo's own controls keep their keys; so does the
  // viewer once they have taken over.
  if (pathHas(e, 'input, textarea, select, [contenteditable], nldd-menu, nldd-menu-item, [role="menu"]')) return;
  if (replay.diverged && !pathHas(e, '.deck, .wt-chrome')) return;
  if (e.key === ' ' && pathHas(e, 'button, a[href], nldd-button, nldd-icon-button')) return;
  const tr = track.value;
  switch (e.key) {
    case ' ':
    case 'k':
      e.preventDefault();
      togglePlay();
      break;
    case 'ArrowLeft':
      e.preventDefault();
      seek(previousChapterStart(tr, replay.now));
      break;
    case 'ArrowRight': {
      e.preventDefault();
      const to = nextChapterStart(tr, replay.now);
      if (to != null) seek(to);
      break;
    }
    case 'j':
      seek(replay.now - 10);
      break;
    case 'l':
      seek(replay.now + 10);
      break;
    case 'c':
      replay.captionsOn = !replay.captionsOn;
      break;
    case 'Escape':
      if (replay.faq) backToMain();
      break;
    default:
  }
}

onMounted(() => {
  raf = requestAnimationFrame(loop);
  window.addEventListener('keydown', onKey);
});
onUnmounted(() => {
  cancelAnimationFrame(raf);
  window.removeEventListener('keydown', onKey);
});
</script>

<template>
  <!-- In <body>, like the deck: inside the workspace this layer would take
       part in its layout and push the demo down. -->
  <Teleport to="body">
  <div class="wt-chrome overlay" :class="{ full }">
    <video
      v-if="track?.cam"
      v-show="replay.camOn"
      ref="cam"
      class="cam"
      :style="camStyle"
      :src="`${MEDIA_BASE}${track.cam.src}`"
      muted
      playsinline
      preload="auto"
      aria-hidden="true"
    ></video>

    <!-- The pointer and the captions go in the browser's top layer, as a
         popover: a sheet or dialog the replay opens lives there too, and
         would otherwise cover the cursor exactly when it clicks inside it.
         Raised again whenever the cursor moves, so it stays on top of a
         dialog that opened after it. -->
    <div ref="pointerLayer" popover="manual" class="pointer-layer" aria-hidden="true">
    <svg v-show="replay.cursor.visible && !replay.diverged" class="cursor" :style="{ transform: `translate(${replay.cursor.x}px, ${replay.cursor.y}px)` }" width="22" height="26" viewBox="0 0 22 26" aria-hidden="true">
      <path d="M2 2 L2 20 L7 15.5 L10.5 23.5 L13.5 22.2 L10 14.4 L17 14.4 Z" />
    </svg>
    <span v-for="r in replay.ripples" :key="r.id" class="ripple" :style="{ left: `${r.x}px`, top: `${r.y}px` }" aria-hidden="true"></span>

    <p v-if="cue" class="caption" lang="nl" aria-hidden="true">{{ cue.text }}</p>
    </div>

    <nldd-sheet ref="sheet" placement="right" width="560px" :accessible-label="t('walkthrough.transcript.title')" @close="transcript.open = false">
      <nldd-page>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="t('walkthrough.transcript.title')" :supporting-text="replay.faq?.question" :dismiss-text="t('walkthrough.transcript.close')" @dismiss="transcript.open = false"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="24" lang="nl">
          <nldd-container v-for="(c, i) in chapters" :key="i" padding="0" gap="8">
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
  </Teleport>
</template>

<style scoped>
/* Custom CSS, on purpose: the design system has no picture-in-picture bubble,
 * no caption overlay and no pointer. The panel is an nldd-sheet. */
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
  pointer-events: none;
}
/* The popover's own box is reset to nothing; its children are fixed to the
   viewport like the rest of the overlay. */
.pointer-layer {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  overflow: visible;
  pointer-events: none;
}
.cursor {
  position: fixed;
  left: 0;
  top: 0;
  z-index: 95;
  margin: -2px 0 0 -2px;
  pointer-events: none;
  transition: transform 0.35s cubic-bezier(0.22, 1, 0.36, 1);
}
.cursor path {
  fill: var(--primitives-color-coolgray-1000);
  stroke: var(--primitives-color-coolgray-0);
  stroke-width: 1.5;
}
.ripple {
  position: fixed;
  z-index: 94;
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
.caption {
  position: fixed;
  z-index: 86;
  left: calc(36vw + 2rem);
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
  pointer-events: none;
}
.full .caption {
  left: 2rem;
  bottom: 12rem;
}
@media (prefers-reduced-motion: reduce) {
  .cam,
  .cursor {
    transition: none;
  }
  .ripple {
    animation-duration: 0.01s;
  }
}
</style>
