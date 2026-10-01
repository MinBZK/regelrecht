<script setup>
/**
 * Where the recorded walkthrough starts.
 *
 * On a wide screen this page hands over to the replay (replay.js): the deck
 * comes up with the presenter's voice and from then on the walkthrough moves
 * through the tabs itself, in the live demo. The page has nothing to show of
 * its own; the first slide covers it.
 *
 * On a phone the demo next to a rail does not fit, so this page plays the
 * recording as a video instead, with its captions.
 */
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useDemo } from '../store/demoStore.js';
import { usePresentation } from '../presentation/usePresentation.js';
import { useI18n } from '../i18n/index.js';
import { localeRouteName } from '../router.js';
import { useWalkthrough } from './useWalkthrough.js';
import { replay, savedPosition, startReplay } from './replay.js';
import { CAPTIONS_BASE, MEDIA_BASE } from './timeline.js';

/** Below this the demo and the rail do not fit side by side (the deck's own breakpoint). */
const WIDE = 1024;

const { t, locale } = useI18n();
const route = useRoute();
const router = useRouter();
const demo = useDemo();
const presentation = usePresentation();
const { timeline, ready } = useWalkthrough();

const loaded = ref(false);
const narrow = ref(false);
const homePath = computed(() => router.resolve({ name: localeRouteName('home', locale.value) }).path);
const video = computed(() => timeline.value?.main?.video ?? null);
const captions = computed(() => timeline.value?.main?.captions?.nl ?? null);

onMounted(async () => {
  await ready;
  await demo.boot().catch(() => {});
  loaded.value = true;
  narrow.value = window.innerWidth < WIDE;
  if (!timeline.value || narrow.value || replay.active) return;
  await startReplay(timeline.value, { router, demo, presentation }, { at: savedPosition(), faqId: route.params.faqId ?? null });
});
</script>

<template>
  <nldd-page v-if="loaded && !timeline">
    <nldd-simple-section width="720px">
      <nldd-title slot="header" size="2">
        <h1>{{ t('walkthrough.missing.title') }}</h1>
        <span slot="subtitle">{{ t('walkthrough.missing.body') }}</span>
      </nldd-title>
      <nldd-button variant="primary" start-icon="home" :text="t('walkthrough.leave')" @click="router.push(homePath)"></nldd-button>
    </nldd-simple-section>
  </nldd-page>

  <nldd-page v-else-if="loaded && narrow">
    <nldd-simple-section>
      <nldd-title slot="header" size="2">
        <h1>{{ t('walkthrough.label') }}</h1>
        <span slot="subtitle">{{ t('walkthrough.phone') }}</span>
      </nldd-title>
      <!-- A phone gets the recording as a video: the live demo next to a
           rail does not fit on it. Native controls, captions as a track. -->
      <video v-if="video" class="phone-video" :src="`${MEDIA_BASE}${video.src}`" controls playsinline preload="metadata" lang="nl">
        <track v-if="captions" kind="captions" srclang="nl" :label="t('walkthrough.captions')" :src="`${CAPTIONS_BASE}${captions}`" default />
      </video>
    </nldd-simple-section>
  </nldd-page>

  <nldd-page v-else>
    <nldd-simple-section height="60vh">
      <nldd-activity-indicator show-text :text="t('app.loading')" timing="instant" size="48"></nldd-activity-indicator>
    </nldd-simple-section>
  </nldd-page>
</template>

<style scoped>
/* Custom CSS: the design system has no video element. Full width, its own
   aspect ratio. */
.phone-video {
  display: block;
  width: 100%;
  height: auto;
  border-radius: 8px;
  background: var(--primitives-color-coolgray-1000);
}
</style>
