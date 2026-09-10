<script setup>
import { computed, onActivated, onMounted, watch } from 'vue';
import { usePresentation } from '../presentation/usePresentation.js';
import { useDemo } from '../store/demoStore.js';

// The Presentatie tab starts the deck. The deck itself is an overlay
// (PresentationDeck.vue, mounted by App.vue) that covers the screen for the
// intro and then sits on the left while it opens the other tabs. This page is
// what remains when the deck is closed on this route: a way to start again.

const p = usePresentation();
const { ready, corpus } = useDemo();
const slides = computed(() => corpus.value?.config?.slides ?? []);

function startWhenReady() {
  if (ready.value && !p.active.value && slides.value.length) p.start(0);
}
onMounted(startWhenReady);
onActivated(startWhenReady);
watch(ready, startWhenReady);

function kindLabel(s) {
  return s.kind === 'title' ? 'Titel' : s.kind === 'statement' ? 'Stelling' : s.kind === 'closing' ? 'Afsluiting' : s.kind === 'section' ? 'Kop' : 'Demo';
}
</script>

<template>
  <nldd-page>
    <nldd-simple-section width="720px">
      <nldd-title slot="header" size="2">
        <span slot="overline">Presentatie</span>
        <h1>RegelRecht, van wet naar digitale werking</h1>
        <span slot="subtitle">De dia's vertellen het verhaal en openen onderweg zelf het juiste tabblad.</span>
        <nldd-container slot="actions" layout="row" gap="8">
          <nldd-button variant="primary" start-icon="play" text="Start de presentatie" :disabled="!ready || undefined" @click="p.start(0)"></nldd-button>
        </nldd-container>
      </nldd-title>
      <nldd-rich-text spacing="tight">
        <p>Pijltjes of spatie bladeren, <kbd>Esc</kbd> sluit de dia's en laat de demo staan, <kbd>f</kbd> zet het scherm vol. Buiten dit tabblad opent <kbd>Shift</kbd>+<kbd>P</kbd> de dia's bij de huidige plek in het verhaal.</p>
      </nldd-rich-text>
      <nldd-list variant="box-base" accessible-label="Dia's">
        <nldd-list-item v-for="(s, i) in slides" :key="i" size="sm" button @click="p.start(i)">
          <nldd-text-cell size="sm" color="secondary" width="fit-content" min-width="32px" :text="String(i + 1)"></nldd-text-cell>
          <nldd-text-cell size="sm" :text="s.title ?? s.lines?.[0]?.replaceAll('**', '') ?? ''" :supporting-text="s.route ? `${kindLabel(s)} · ${s.route}` : kindLabel(s)"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-simple-section>
  </nldd-page>
</template>
