<template>
  <div class="assistent">
    <nldd-banner v-if="health === 'onbereikbaar'" variant="accent">
      De assistent-backend draait niet. Start hem met <code>just poc-assistent nieuwkomersbekostiging</code> in een tweede terminal.
    </nldd-banner>
    <nldd-banner v-else-if="health === 'geen-cli'" variant="warning">
      De backend draait, maar de Claude Code CLI is niet gevonden. Installeer die en herstart met <code>just poc-assistent nieuwkomersbekostiging</code>.
    </nldd-banner>

    <div class="as-modus">
      <nldd-segmented-control :value="modus" @change="modus = $event.detail?.value ?? modus">
        <nldd-segmented-control-item value="instructie" text="Instructie"></nldd-segmented-control-item>
        <nldd-segmented-control-item value="doel" text="Doel"></nldd-segmented-control-item>
      </nldd-segmented-control>
    </div>

    <nldd-form-field :label="modus === 'doel' ? 'Beschrijf het beleidsdoel' : 'Geef een instructie'">
      <nldd-multi-line-text-field
        :value="prompt"
        :placeholder="placeholder"
        :disabled="!beschikbaar || streaming"
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
          📊 Simulatie ({{ item.doel }}<span v-if="item.n">, n={{ item.n }}</span>): {{ item.samenvatting }}
        </template>
        <template v-else-if="item.type === 'fout'">⚠️ {{ item.melding }}</template>
      </div>
    </div>

    <nldd-banner v-if="resultaat && !heeftWijziging" variant="accent">
      De assistent heeft niets gewijzigd.
    </nldd-banner>
    <nldd-button
      v-if="heeftWijziging"
      :text="`Overnemen in ${werkversieLabel}`"
      start-icon="save"
      variant="secondary"
      @click="takeOverlays"
    ></nldd-button>
    <p v-if="heeftWijziging" class="as-hint">
      Zet de wijziging van de assistent als bewerking in de werkversie ({{ wijzigingLabel }}); zichtbaar via Bekijk
      wijzigingen, terug te draaien met Terugzetten, te bewaren met Bewaar als variant.
    </p>
  </div>
</template>

<script setup>
import { ref, computed, nextTick, onMounted, onUnmounted } from 'vue';
import { useAssistent } from '../../composables/useAssistent.js';
import { useHandelingen } from '../../composables/useHandelingen.js';
import { useLawStore } from '../../engine/lawStore.js';
import { euroCompact } from '../../lib/format.js';
import { b } from '../../basePad.js';

const emit = defineEmits(['metrics']);

const { streaming, run, abort } = useAssistent();

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
const { applyOverlays, lawDocsFor, werkversie, werkversieLabel } = useLawStore();
const { applyHandelingenYaml, ensureVariantDoc, handelingenYamlFor } = useHandelingen();

// 'onbekend' (nog niet gepeild) | 'onbereikbaar' | 'geen-cli' | 'ok'
const health = ref('onbekend');
const beschikbaar = computed(() => health.value === 'ok');
let healthTimer = null;

/**
 * Peil /api/health rechtstreeks (de composable levert alleen ok, niet het
 * cli-veld). Blijft elke 10s pollen zolang de backend niet volledig
 * beschikbaar is, zodat het paneel vanzelf herstelt als de server start.
 */
async function peilHealth() {
  try {
    const res = await fetch(b('/api/health'), { signal: AbortSignal.timeout(3000) });
    if (!res.ok) throw new Error(String(res.status));
    const data = await res.json();
    health.value = data.cli ? 'ok' : 'geen-cli';
  } catch {
    health.value = 'onbereikbaar';
  }
  if (health.value !== 'ok' && !healthTimer) {
    healthTimer = setInterval(peilHealth, 10000);
  } else if (health.value === 'ok' && healthTimer) {
    clearInterval(healthTimer);
    healthTimer = null;
  }
  return health.value;
}

const modus = ref('instructie');
const prompt = ref('');
const feed = ref([]);
const overlays = ref(null);
// De bewerkte handelingen.yaml, als de assistent het uitvoeringslastmodel
// raakte. Apart van de overlays: dat zijn wetten, dit is het kostenmodel.
const handelingenYaml = ref(null);
// Is er een resultaat binnen? Onderscheidt "nog niets gedraaid" van "gedraaid
// en niets gewijzigd"; zonder dat verscheen de banner ook voor de eerste run.
const resultaat = ref(false);
const feedEl = ref(null);

const aantalDocumenten = computed(() => Object.keys(overlays.value ?? {}).length);
const heeftWijziging = computed(() => aantalDocumenten.value > 0 || !!handelingenYaml.value);
const wijzigingLabel = computed(() => {
  const delen = [];
  if (aantalDocumenten.value) {
    delen.push(`${aantalDocumenten.value} ${aantalDocumenten.value === 1 ? 'document' : 'documenten'}`);
  }
  if (handelingenYaml.value) delen.push('het uitvoeringslastmodel');
  return delen.join(' en ');
});

const placeholder = computed(() =>
  modus.value === 'doel'
    ? 'Trek de bedragen voor asielzoekers en overige vreemdelingen in het po gelijk zonder dat de totale uitgave stijgt'
    : 'Laat de drempel van vier nieuwkomers per school in artikel 34 vervallen',
);

onMounted(peilHealth);
onUnmounted(() => {
  if (healthTimer) clearInterval(healthTimer);
});

function samenvatting(metrics) {
  if (!metrics) return 'klaar';
  const t = metrics.totaal ?? metrics;
  const delen = [];
  if (t.regeling_po != null) delen.push(`po ${euroCompact(t.regeling_po)}`);
  if (t.regeling_vo != null) delen.push(`vo ${euroCompact(t.regeling_vo)}`);
  if (t.uitvoeringslast != null) delen.push(`uitvoeringslast ${euroCompact(t.uitvoeringslast)}`);
  return delen.length ? `${delen.join(' · ')} per jaar` : 'klaar';
}

async function submit() {
  feed.value = [];
  overlays.value = null;
  handelingenYaml.value = null;
  resultaat.value = false;

  // De assistent werkt op de werkversie: stuur die documenten mee als beginstand.
  const documenten = (await lawDocsFor(werkversie.value)).map((d) => ({ key: `${d.entry.id}@${d.entry.valid_from ?? ''}`, yaml: d.yaml }));
  // Ook het uitvoeringslastmodel van deze kolom: een variant kan er een eigen
  // meebrengen, en zonder dit werkt de assistent op de basis.
  await ensureVariantDoc(werkversie.value);
  const handelingen = handelingenYamlFor(werkversie.value);
  feed.value.push({ type: 'tekst', tekst: `Werkt op ${werkversieLabel.value}.` });
  await run({ modus: modus.value, prompt: prompt.value, documenten, handelingen }, (ev) => {
    if (ev.type === 'tool') {
      const inputText = ev.input
        ? Object.entries(ev.input).map(([k, v]) => `${k}=${v}`).join(' ')
        : '';
      feed.value.push({ type: 'tool', naam: ev.naam, inputText });
    } else if (ev.type === 'simulatie') {
      feed.value.push({ type: 'simulatie', doel: ev.doel, n: ev.n, samenvatting: samenvatting(ev.metrics) });
      if (ev.metrics) emit('metrics', ev.metrics);
    } else if (ev.type === 'klaar') {
      overlays.value = ev.overlays ?? null;
      handelingenYaml.value = ev.handelingen ?? null;
      resultaat.value = true;
    } else {
      feed.value.push(ev);
    }
    nextTick(() => {
      if (feedEl.value) feedEl.value.scrollTop = feedEl.value.scrollHeight;
    });
  });
}

function takeOverlays() {
  if (!heeftWijziging.value) return;
  if (aantalDocumenten.value) applyOverlays(overlays.value);
  // Een onleesbaar model niet stil laten vallen: dan blijft de knop staan en
  // weet de gebruiker dat er niets is overgenomen.
  if (handelingenYaml.value && !applyHandelingenYaml(handelingenYaml.value)) {
    feed.value.push({ type: 'fout', melding: 'Het uitvoeringslastmodel van de assistent was onleesbaar en is niet overgenomen.' });
    return;
  }
  feed.value.push({ type: 'tekst', tekst: `Overgenomen in ${werkversieLabel.value}.` });
  overlays.value = null;
  handelingenYaml.value = null;
}
</script>

<style scoped>
.assistent { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.as-modus { display: flex; }
.as-feed {
  display: flex;
  flex-direction: column;
  gap: var(--primitives-space-8);
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
.as-md code { font-size: 0.9em; }
</style>
