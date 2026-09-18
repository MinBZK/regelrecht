<script setup>
import { computed, onActivated, onMounted, watch } from 'vue';
import { usePresentation } from '../presentation/usePresentation.js';
import { useDemo } from '../store/demoStore.js';

// The Presentatie tab starts the deck. The deck itself is an overlay
// (PresentationDeck.vue, mounted by App.vue) that covers the screen for the
// intro and then sits on the left while it opens the other tabs. This page is
// what remains when the deck is closed on this route: a way to start again.

const p = usePresentation();
const { ready, corpus, state } = useDemo();
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
        <!-- `end`, niet `actions`: nldd-title heeft geen actions-slot, en de
             knop viel daardoor buiten de shadow-DOM (0x0, onzichtbaar). Zonder
             container ertussen, want die krijgt in `.title__end` geen breedte. -->
        <nldd-button slot="end" variant="primary" start-icon="play" text="Start de presentatie" :disabled="!ready || undefined" @click="p.start(0)"></nldd-button>
      </nldd-title>
      <nldd-rich-text spacing="tight">
        <p>Pijltjes of spatie bladeren, <kbd>Esc</kbd> sluit de dia's en laat de demo staan, <kbd>f</kbd> zet het scherm vol. Buiten dit tabblad opent <kbd>Shift</kbd>+<kbd>P</kbd> de dia's bij de huidige plek in het verhaal.</p>
      </nldd-rich-text>
      <!-- De modus bepaalt of de dia's náást de demo blijven staan. In de zaal
           vertelt de presentator zelf en is het scherm van de demo; zelfstandig
           is er niemand die het verhaal erbij vertelt, dus blijft het staan.
           `nldd-segmented-control` zoals in WettenView; de uitleg eronder in
           dezelfde rich-text als de toetsenregel hierboven. -->
      <nldd-container padding="0" gap="8">
        <nldd-segmented-control
          width="fit-content"
          accessible-label="Hoe wordt er gepresenteerd?"
          :value="state.presentationMode"
          @change="state.presentationMode = $event.detail?.value ?? state.presentationMode"
        >
          <nldd-segmented-control-item value="zaal" text="In de zaal"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="zelfstandig" text="Zelfstandig"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-rich-text spacing="tight">
          <p v-if="state.presentationMode === 'zaal'">
            De dia's van het verhaal vullen het scherm. Zodra een dia de demo opent, verdwijnen ze en is het scherm van de demo. Bladeren gaat daar gewoon door. Loop je zelf naar een ander tabblad, dan laten de dia's het toetsenbord los; <kbd>Shift</kbd>+<kbd>P</kbd> haalt ze terug.
          </p>
          <p v-else>De dia's blijven links naast de demo staan, zodat iemand die zelf doorklikt het verhaal erbij leest.</p>
        </nldd-rich-text>
      </nldd-container>
      <nldd-list variant="box-base" accessible-label="Dia's">
        <nldd-list-item v-for="(s, i) in slides" :key="i" size="sm" button @click="p.start(i)">
          <nldd-text-cell size="sm" color="secondary" width="fit-content" min-width="32px" :text="String(i + 1)"></nldd-text-cell>
          <nldd-text-cell size="sm" :text="s.title ?? s.lines?.[0]?.replaceAll('**', '') ?? ''" :supporting-text="s.route ? `${kindLabel(s)} · ${s.route}` : kindLabel(s)"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-simple-section>
  </nldd-page>
</template>
