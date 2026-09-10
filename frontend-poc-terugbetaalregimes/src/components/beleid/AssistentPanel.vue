<template>
  <div class="assistent">
    <nldd-banner v-if="health === 'onbereikbaar'" variant="accent">
      De assistent-backend draait niet. Start hem met <code>just server</code> in een tweede terminal.
    </nldd-banner>
    <nldd-banner v-else-if="health === 'geen-cli'" variant="warning">
      De backend draait, maar de Claude Code CLI is niet gevonden. Installeer die en herstart met <code>just server</code>.
    </nldd-banner>

    <nldd-segmented-control size="sm" :value="modus" @change="modus = $event.detail?.value ?? modus">
      <nldd-segmented-control-item value="instructie" text="Instructie"></nldd-segmented-control-item>
      <nldd-segmented-control-item value="doel" text="Doel"></nldd-segmented-control-item>
    </nldd-segmented-control>

    <nldd-form-field :label="modus === 'doel' ? 'Doel' : 'Instructie'">
      <nldd-multi-line-text-field
        :value="prompt"
        :placeholder="placeholder"
        rows="3"
        @input="prompt = $event.detail?.value ?? prompt"
      ></nldd-multi-line-text-field>
    </nldd-form-field>

    <div class="as-knoppen">
      <nldd-button
        :text="streaming ? 'Assistent werkt…' : 'Verstuur'"
        start-icon="send"
        variant="primary"
        :disabled="!beschikbaar || streaming || !prompt.trim()"
        @click="submit"
      ></nldd-button>
      <nldd-button v-if="streaming" text="Stop" start-icon="remove" variant="secondary" @click="stop"></nldd-button>
    </div>

    <optimalisatiepad-chart v-if="modus === 'doel' && pad.length" :pad="pad" />

    <div v-if="feed.length" ref="feedEl" class="as-feed">
      <div v-for="(item, i) in feed" :key="i" class="as-item" :class="`as-${item.type}`">
        <span v-if="item.type === 'tekst'" class="as-md" v-html="eenvoudigeMarkdown(item.tekst)"></span>
        <template v-else-if="item.type === 'tool'">
          🔧 {{ item.naam }}<span v-if="item.inputText"> {{ item.inputText }}</span>
        </template>
        <template v-else-if="item.type === 'wijziging'">
          ✏️ <strong>{{ item.document_key }}</strong>: {{ item.toelichting }}
        </template>
        <template v-else-if="item.type === 'simulatie'">
          📊 Simulatie ({{ item.doel }}<span v-if="item.n">, n={{ item.n }}</span>):
          {{ item.pct !== null ? percent(item.pct, 1) + ' betalingsproblemen' : 'klaar' }}
        </template>
        <template v-else-if="item.type === 'fout'">⚠️ {{ item.melding }}</template>
      </div>
    </div>

    <nldd-banner v-if="overlays && !Object.keys(overlays).length" variant="accent">
      De assistent heeft niets gewijzigd.
    </nldd-banner>
    <nldd-button
      v-if="overlays && Object.keys(overlays).length"
      :text="`Overnemen in ${werkversieLabel}`"
      start-icon="save"
      variant="secondary"
      @click="takeOverlays"
    ></nldd-button>
    <p v-if="overlays && Object.keys(overlays).length" class="as-hint">
      Zet de wijziging van de assistent als bewerking in de werkversie ({{ Object.keys(overlays).length }}
      {{ Object.keys(overlays).length === 1 ? 'document' : 'documenten' }}); zichtbaar via Bekijk wijzigingen, terug te
      draaien met Terugzetten, te bewaren met Bewaar als variant.
    </p>
  </div>
</template>

<script setup>
import { ref, computed, nextTick, onMounted, onUnmounted } from 'vue';
import { useAssistent } from '../../composables/useAssistent.js';
import { useLawStore } from '../../engine/lawStore.js';
import { percent } from '../../lib/format.js';
import { b } from '../../basePad.js';
import OptimalisatiepadChart from './OptimalisatiepadChart.vue';

const emit = defineEmits(['metrics']);

const { streaming, run, abort } = useAssistent();
const { applyOverlays, lawDocsFor, werkversie, werkversieLabel } = useLawStore();

/** Minimale markdown voor de assistenttekst: vet, code, regeleinden; de rest blijft tekst. */
function eenvoudigeMarkdown(tekst) {
  const esc = String(tekst ?? '').replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
  return esc
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\n/g, '<br>');
}

function stop() {
  abort();
  feed.value.push({ type: 'tekst', tekst: 'Afgebroken.' });
}

// 'onbekend' (nog niet gepeild) | 'onbereikbaar' | 'geen-cli' | 'ok'
const health = ref('onbekend');
const beschikbaar = computed(() => health.value === 'ok');
let healthTimer = null;

async function peilHealth() {
  try {
    const res = await fetch(b('/api/health'));
    if (!res.ok) throw new Error();
    const body = await res.json();
    health.value = body.cli ? 'ok' : 'geen-cli';
  } catch {
    health.value = 'onbereikbaar';
  }
  // Blijf peilen als de backend (nog) niet bereikbaar is, zodat "just server"
  // starten in een tweede terminal het paneel vanzelf ontgrendelt.
  if (health.value !== 'ok' && !healthTimer) {
    healthTimer = setInterval(async () => {
      await peilHealth();
      if (health.value === 'ok') {
        clearInterval(healthTimer);
        healthTimer = null;
      }
    }, 5000);
  }
}

const modus = ref('instructie');
const prompt = ref('');
const feed = ref([]);
const pad = ref([]); // [{iteratie, pct}] voor de doel-modus
const overlays = ref(null);
const feedEl = ref(null);

const placeholder = computed(() =>
  modus.value === 'doel'
    ? 'Minimaliseer het aantal debiteuren met betalingsproblemen zonder de kwijtscheldingskosten meer dan te verdubbelen'
    : 'Verhoog de draagkrachtvrije voet van SF15-oud naar 84% van het belastbaar minimumloon',
);

onMounted(peilHealth);
onUnmounted(() => {
  if (healthTimer) clearInterval(healthTimer);
});

async function submit() {
  feed.value = [];
  pad.value = [];
  overlays.value = null;
  let iteratie = 0;

  // De assistent werkt op de werkversie: stuur die documenten mee als beginstand.
  const documenten = (await lawDocsFor(werkversie.value)).map((d) => ({ key: `${d.entry.id}@${d.entry.valid_from ?? ''}`, yaml: d.yaml }));
  feed.value.push({ type: 'tekst', tekst: `Werkt op ${werkversieLabel.value}.` });
  await run({ modus: modus.value, prompt: prompt.value, documenten }, (ev) => {
    if (ev.type === 'tool') {
      const inputText = ev.input
        ? Object.entries(ev.input).map(([k, v]) => `${k}=${v}`).join(' ')
        : '';
      feed.value.push({ type: 'tool', naam: ev.naam, inputText });
    } else if (ev.type === 'simulatie') {
      const pct = ev.metrics?.pctBetalingsprobleem ?? null;
      feed.value.push({ type: 'simulatie', doel: ev.doel, n: ev.n, pct });
      if (ev.metrics) emit('metrics', ev.metrics);
      if (ev.doel === 'populatie' && pct !== null) {
        pad.value.push({ iteratie: ++iteratie, pct });
      }
    } else if (ev.type === 'klaar') {
      overlays.value = ev.overlays ?? null;
    } else {
      feed.value.push(ev);
    }
    nextTick(() => {
      if (feedEl.value) feedEl.value.scrollTop = feedEl.value.scrollHeight;
    });
  });
}

function takeOverlays() {
  if (!overlays.value) return;
  applyOverlays(overlays.value);
  feed.value.push({ type: 'tekst', tekst: `Overgenomen in ${werkversieLabel.value}.` });
  overlays.value = null;
}
</script>

<style scoped>
.assistent { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.as-feed {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 280px;
  overflow-y: auto;
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  border: 1px solid var(--semantics-dividers-color);
}
.as-item { font-size: 0.9em; line-height: 1.45; }
.as-tekst { color: var(--semantics-content-color); }
.as-tool { color: var(--semantics-content-secondary-color); font-family: var(--primitives-font-family-monospace, monospace); font-size: 0.85em; }
.as-wijziging { color: var(--semantics-content-accent-color); }
.as-simulatie { color: var(--semantics-content-secondary-color); }
.as-fout { color: var(--semantics-content-critical-color); }
.as-knoppen { display: flex; gap: var(--primitives-space-8); align-items: center; }
.as-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.as-md code { font-size: 0.9em; }
</style>
