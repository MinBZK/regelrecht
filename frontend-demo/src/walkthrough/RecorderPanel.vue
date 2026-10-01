<script setup>
/**
 * The recorder's controls, shown in dev with `?record` in the address.
 *
 * The take is the whole window, so the panel leaves the screen while it
 * runs: the tab title shows that it is recording, Shift+X marks a slip and
 * Shift+R stops. It is recorded in zaal mode, the way the presenter works in
 * a room: the deck covers the screen on a story slide and steps aside on a
 * demo slide.
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useDemo } from '../store/demoStore.js';
import { usePresentation } from '../presentation/usePresentation.js';
import { useI18n } from '../i18n/index.js';
import { hasSnapshots, logEvent, markFlub, recorder, relativePoint, rememberSnapshot, snapshotFor, startRecording, stopRecording } from './recorder.js';

const { t } = useI18n();
const router = useRouter();
const demo = useDemo();
const p = usePresentation();

const startAt = ref(0);
const restore = ref(hasSnapshots());
const slides = computed(() => p.slides.value ?? []);

const cleanups = [];
let previousMode = null;
let previousTitle = null;

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}

function onPointer(e) {
  const point = relativePoint({ left: 0, top: 0, width: window.innerWidth, height: window.innerHeight }, e.clientX, e.clientY);
  if (point) logEvent({ type: 'click', ...point });
}

function editable(e) {
  return (e.composedPath?.() ?? []).some((n) => n?.matches?.('input, textarea, select, [contenteditable]'));
}

function onKey(e) {
  if (e.key === 'X' && e.shiftKey && !editable(e)) {
    e.preventDefault();
    markFlub();
    return;
  }
  // Only that someone typed, never what: the log marks stretches the cutter
  // must leave whole, and has no use for the characters.
  if (editable(e) && e.key.length === 1) logEvent({ type: 'key' });
}

function listen() {
  document.addEventListener('pointerdown', onPointer, true);
  window.addEventListener('keydown', onKey, true);
  cleanups.push(() => document.removeEventListener('pointerdown', onPointer, true));
  cleanups.push(() => window.removeEventListener('keydown', onKey, true));
  cleanups.push(router.afterEach((to) => logEvent({ type: 'route', path: to.fullPath })));
  cleanups.push(
    watch(
      [p.index, p.active],
      ([index, active]) => {
        if (!active) {
          logEvent({ type: 'deck', active: false });
          return;
        }
        const state = plain(demo.state);
        rememberSnapshot(index, state);
        logEvent({ type: 'slide', index, slide: plain(p.current.value ?? {}), profile: demo.profileKey.value, state });
      },
      { immediate: true },
    ),
  );
}

function unlisten() {
  while (cleanups.length) cleanups.pop()();
}

async function start() {
  if (restore.value) {
    const snap = snapshotFor(startAt.value);
    if (snap) {
      Object.assign(demo.state, snap);
      demo.reregister();
    }
  }
  previousMode = demo.state.presentationMode;
  demo.state.presentationMode = 'zaal';
  const takeId = await startRecording({
    meta: {
      startSlide: startAt.value,
      slideCount: slides.value.length,
      viewport: { width: window.innerWidth, height: window.innerHeight, dpr: window.devicePixelRatio },
      locale: document.documentElement.lang || null,
      userAgent: navigator.userAgent,
    },
  });
  if (!takeId) {
    demo.state.presentationMode = previousMode;
    return;
  }
  // The panel is off screen now; the tab title, outside the picture, says
  // that the take runs.
  previousTitle = document.title;
  document.title = `● REC · ${previousTitle}`;
  listen();
  p.start(startAt.value);
}

function restoreAfterTake() {
  unlisten();
  if (previousTitle != null) document.title = previousTitle;
  previousTitle = null;
  if (previousMode) demo.state.presentationMode = previousMode;
  previousMode = null;
}

async function stop() {
  await stopRecording();
  restoreAfterTake();
}

// Chrome's own "stop sharing" ends the take from outside this panel.
watch(
  () => recorder.phase,
  (phase) => {
    if (phase === 'saving' || phase === 'error') restoreAfterTake();
  },
);

function onGlobalKey(e) {
  // Shift+R stops, so the presenter never has to reach for the mouse in the
  // middle of a sentence (and the panel is off screen anyway).
  if (e.key === 'R' && e.shiftKey && !editable(e)) {
    e.preventDefault();
    if (recorder.phase === 'recording') stop();
  }
}
onMounted(() => window.addEventListener('keydown', onGlobalKey));
onUnmounted(() => {
  window.removeEventListener('keydown', onGlobalKey);
  unlisten();
});
</script>

<template>
  <div v-if="recorder.phase !== 'recording'" class="recorder" role="region" :aria-label="t('recorder.label')">
    <nldd-card background="base">
      <nldd-container padding="16" gap="12">
        <template v-if="recorder.phase === 'saving'">
          <nldd-activity-indicator show-text :text="t('recorder.saving')" timing="instant" size="24"></nldd-activity-indicator>
        </template>
        <template v-else>
          <nldd-text size="md">{{ t('recorder.title') }}</nldd-text>
          <nldd-form-field :label="t('recorder.start_at')">
            <nldd-dropdown width="full">
              <select :value="startAt" @change="startAt = Number($event.target.value)">
                <option v-for="(s, i) in slides" :key="i" :value="i">{{ i + 1 }}. {{ s.title ?? s.lines?.[0]?.replaceAll('**', '') ?? '' }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <nldd-switch-field :label="t('recorder.camera')" :checked="recorder.withCamera || undefined" @change="recorder.withCamera = !recorder.withCamera"></nldd-switch-field>
          <nldd-switch-field :label="t('recorder.restore')" :checked="restore || undefined" @change="restore = !restore"></nldd-switch-field>
          <nldd-text v-if="recorder.phase === 'done'" size="sm" color="secondary">{{ t('recorder.saved', { take: recorder.takeId }) }}</nldd-text>
          <nldd-text v-if="recorder.error" size="sm" color="critical">{{ recorder.errorKey ? t(recorder.errorKey) : recorder.error }}</nldd-text>
          <nldd-button variant="primary" start-icon="play" :text="t('recorder.start')" :loading="recorder.phase === 'starting' || undefined" :disabled="!slides.length || undefined" @click="start"></nldd-button>
          <nldd-text size="sm" color="secondary">{{ t('recorder.help') }}</nldd-text>
        </template>
      </nldd-container>
    </nldd-card>
  </div>
</template>

<style scoped>
/* Custom CSS: the design system has no floating panel. Position only; the
   panel itself is an nldd-card. Over the rail, never over the captured area. */
.recorder {
  position: fixed;
  top: 0.75rem;
  left: 0.75rem;
  width: min(22rem, calc(36vw - 1.5rem));
  z-index: 95;
}
</style>
