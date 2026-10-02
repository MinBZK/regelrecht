<script setup>
/**
 * Under the slide in the rail, while a walkthrough runs: the spot for the
 * presenter's bubble and the questions offered so far.
 */
import { computed, onUnmounted, ref, watch } from 'vue';
import { useI18n } from '../i18n/index.js';
import { useWalkthrough } from './useWalkthrough.js';
import { currentTrack, openFaq, replay } from './replay.js';
import { camVisible, offeredFaq } from './timeline.js';
import { camSlot } from './chrome.js';

const { t, locale } = useI18n();
const { timeline } = useWalkthrough();

const slot = ref(null);
watch(slot, (el) => (camSlot.value = el), { immediate: true });
onUnmounted(() => (camSlot.value = null));

const showCam = computed(() => camVisible(currentTrack(), replay.now) && replay.camOn);
const offered = computed(() => (replay.faq ? [] : offeredFaq(timeline.value, replay.now)));
</script>

<template>
  <div v-if="showCam || offered.length" class="aside wt-chrome">
    <div v-if="showCam" ref="slot" class="cam-slot" aria-hidden="true"></div>
    <div v-if="offered.length" class="faq" lang="nl" role="region" :aria-label="t('walkthrough.faq.label')">
      <span class="faq-title" :lang="locale">{{ t('walkthrough.faq.title') }}</span>
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

<style scoped>
/* Custom CSS: a reserved spot for the bubble and the list of questions
   appearing. The questions themselves are nldd buttons. */
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
@media (prefers-reduced-motion: reduce) {
  .faq-enter-active {
    transition: none;
  }
}
</style>
