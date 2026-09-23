<script setup>
import { computed, onActivated, onMounted, watch } from 'vue';
import { usePresentation } from '../presentation/usePresentation.js';
import { useDemo } from '../store/demoStore.js';
import { useI18n } from '../i18n/index.js';

// The Presentatie tab starts the deck. The deck itself is an overlay
// (PresentationDeck.vue, mounted by App.vue) that covers the screen for the
// intro and then sits on the left while it opens the other tabs. This page is
// what remains when the deck is closed on this route: a way to start again.

const p = usePresentation();
const { t } = useI18n();
const { ready, corpus, state } = useDemo();
const slides = computed(() => corpus.value?.config?.slides ?? []);

// De toetsen staan als `{esc}`, `{f}` en `{shiftp}` in de zin en worden hier
// tot `<kbd>` gevuld. De woordenboeken houden dan hele zinnen in plaats van
// stukjes rond een `<kbd>`, en een taal die de toetsen ergens anders in de zin
// zet kan dat ook doen. De invulling is onze eigen tekst, geen invoer, dus
// `v-html` is hier hetzelfde als in PresentationDeck.vue.
const KEYS = {
  esc: '<kbd>Esc</kbd>',
  f: '<kbd>f</kbd>',
  shiftp: '<kbd>Shift</kbd>+<kbd>P</kbd>',
};
const keyHelp = computed(() => t('home.presentation.keys', KEYS));
const modeHelp = computed(() =>
  state.presentationMode === 'zaal' ? t('home.presentation.mode.zaal.body', KEYS) : t('home.presentation.mode.zelfstandig.body'),
);

function startWhenReady() {
  if (ready.value && !p.active.value && slides.value.length) p.start(0);
}
onMounted(startWhenReady);
onActivated(startWhenReady);
watch(ready, startWhenReady);

// Een dia zonder eigen soort is er een die de demo opent; die heet naar de demo
// zelf, dezelfde sleutel als het menu in de werkbalk.
const SLIDE_KINDS = new Set(['title', 'statement', 'closing', 'section']);
function kindLabel(s) {
  return SLIDE_KINDS.has(s.kind) ? t(`home.presentation.kind.${s.kind}`) : t('app.demo.label');
}
</script>

<template>
  <nldd-page>
    <nldd-simple-section width="720px">
      <nldd-title slot="header" size="2">
        <span slot="overline">{{ t('home.presentation.overline') }}</span>
        <h1>{{ t('home.presentation.title') }}</h1>
        <span slot="subtitle">{{ t('home.presentation.subtitle') }}</span>
        <!-- `end`, niet `actions`: nldd-title heeft geen actions-slot, en de
             knop viel daardoor buiten de shadow-DOM (0x0, onzichtbaar). Zonder
             container ertussen, want die krijgt in `.title__end` geen breedte. -->
        <nldd-button slot="end" variant="primary" start-icon="play" :text="t('home.presentation.start')" :disabled="!ready || undefined" @click="p.start(0)"></nldd-button>
      </nldd-title>
      <nldd-rich-text spacing="tight">
        <p v-html="keyHelp"></p>
      </nldd-rich-text>
      <!-- De modus bepaalt of de dia's náást de demo blijven staan. In de zaal
           vertelt de presentator zelf en is het scherm van de demo; zelfstandig
           is er niemand die het verhaal erbij vertelt, dus blijft het staan.
           `nldd-segmented-control` zoals in WettenView; de uitleg eronder in
           dezelfde rich-text als de toetsenregel hierboven. -->
      <nldd-container padding="0" gap="8">
        <nldd-segmented-control
          width="fit-content"
          :accessible-label="t('home.presentation.mode.label')"
          :value="state.presentationMode"
          @change="state.presentationMode = $event.detail?.value ?? state.presentationMode"
        >
          <nldd-segmented-control-item value="zaal" :text="t('home.presentation.mode.zaal')"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="zelfstandig" :text="t('home.presentation.mode.zelfstandig')"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-rich-text spacing="tight">
          <p v-html="modeHelp"></p>
        </nldd-rich-text>
      </nldd-container>
      <nldd-list variant="box-base" :accessible-label="t('home.presentation.slides.label')">
        <nldd-list-item v-for="(s, i) in slides" :key="i" size="sm" button @click="p.start(i)">
          <nldd-text-cell size="sm" color="secondary" width="fit-content" min-width="32px" :text="String(i + 1)"></nldd-text-cell>
          <nldd-text-cell size="sm" :text="s.title ?? s.lines?.[0]?.replaceAll('**', '') ?? ''" :supporting-text="s.route ? `${kindLabel(s)} · ${s.route}` : kindLabel(s)"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-simple-section>
  </nldd-page>
</template>
