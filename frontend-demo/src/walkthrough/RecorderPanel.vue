<script setup>
/**
 * The recorder's controls, shown in dev with `?record` in the address.
 *
 * Sits in the top-left corner, over the deck rail. The capture is cropped to
 * the workspace next to the rail, so nothing here ends up in the recording.
 * While a take runs the body keeps the rail's offset on every slide (see
 * `rr-recording` in presentation.css): the captured area must not change size
 * halfway, or the video changes resolution under the encoder.
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
const minutes = computed(() => {
  const s = Math.floor(recorder.elapsed / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
});
const levelPct = computed(() => Math.min(100, Math.round(recorder.level * 100)));

function captureEl() {
  return document.querySelector('nldd-bar-split-view');
}

const cleanups = [];
let previousMode = null;

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}

function onPointer(e) {
  const el = captureEl();
  const point = relativePoint(el?.getBoundingClientRect(), e.clientX, e.clientY);
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
  const el = captureEl();
  if (!el) return;
  if (restore.value) {
    const snap = snapshotFor(startAt.value);
    if (snap) {
      Object.assign(demo.state, snap);
      demo.reregister();
    }
  }
  previousMode = demo.state.presentationMode;
  demo.state.presentationMode = 'zelfstandig';
  document.documentElement.classList.add('rr-recording');
  const takeId = await startRecording({
    captureEl: el,
    meta: {
      startSlide: startAt.value,
      slideCount: slides.value.length,
      viewport: { width: window.innerWidth, height: window.innerHeight, dpr: window.devicePixelRatio },
      locale: document.documentElement.lang || null,
      userAgent: navigator.userAgent,
    },
  });
  if (!takeId) {
    document.documentElement.classList.remove('rr-recording');
    demo.state.presentationMode = previousMode;
    return;
  }
  listen();
  p.start(startAt.value);
}

async function stop() {
  unlisten();
  await stopRecording();
  document.documentElement.classList.remove('rr-recording');
  if (previousMode) demo.state.presentationMode = previousMode;
}

// Chrome's own "stop sharing" ends the take from outside this panel.
watch(
  () => recorder.phase,
  (phase) => {
    if (phase === 'saving' || phase === 'error') {
      unlisten();
      document.documentElement.classList.remove('rr-recording');
    }
  },
);

function onGlobalKey(e) {
  // Shift+R starts and stops, so the presenter never has to reach for the mouse
  // in the middle of a sentence.
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
  <div class="recorder" role="region" :aria-label="t('recorder.label')">
    <nldd-card background="base">
      <nldd-container padding="16" gap="12">
        <template v-if="recorder.phase === 'recording' || recorder.phase === 'saving'">
          <nldd-container layout="horizontal" gap="8" vertical-alignment="center">
            <span class="dot" aria-hidden="true"></span>
            <nldd-text size="md">{{ t('recorder.recording', { time: minutes }) }}</nldd-text>
            <nldd-tag size="sm" :text="t('recorder.slide', { n: p.index.value + 1, total: slides.length })"></nldd-tag>
            <nldd-tag v-if="recorder.flubs" size="sm" color="warning" :text="t.plural(recorder.flubs, 'recorder.flubs')"></nldd-tag>
          </nldd-container>
          <nldd-progress-bar size="sm" :value="levelPct" max="100" :accessible-label="t('recorder.level')"></nldd-progress-bar>
          <nldd-button-bar>
            <nldd-button size="sm" variant="secondary" start-icon="flag" :text="t('recorder.flub')" @click="markFlub"></nldd-button>
            <nldd-button size="sm" variant="destructive" start-icon="stop" :text="t('recorder.stop')" :loading="recorder.phase === 'saving' || undefined" @click="stop"></nldd-button>
          </nldd-button-bar>
          <nldd-text size="sm" color="secondary">{{ t('recorder.keys') }}</nldd-text>
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
.dot {
  width: 0.75rem;
  height: 0.75rem;
  border-radius: 50%;
  background: var(--primitives-color-rood-500);
  animation: rr-rec-blink 1.2s steps(2, start) infinite;
}
@keyframes rr-rec-blink {
  to {
    visibility: hidden;
  }
}
</style>
