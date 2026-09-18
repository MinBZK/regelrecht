<template>
  <div class="assistent">
    <nldd-banner v-if="health === 'onbereikbaar'" variant="accent">
      De assistent-backend draait niet. Start hem met <code>just poc-assistent nieuwkomersbekostiging</code> in een tweede terminal.
    </nldd-banner>
    <nldd-banner v-else-if="health === 'geen-cli'" variant="warning">
      De backend draait, maar de Claude Code CLI is niet gevonden. Installeer die en herstart met <code>just poc-assistent nieuwkomersbekostiging</code>.
    </nldd-banner>

    <!-- Doel staat voor en is de standaard: dat is de vraag waarmee een
         beleidsmaker binnenkomt. -->
    <div class="as-modus">
      <nldd-segmented-control :value="modus" @change="modus = $event.detail?.value ?? modus">
        <nldd-segmented-control-item value="doel" text="Doel"></nldd-segmented-control-item>
        <nldd-segmented-control-item value="instructie" text="Instructie"></nldd-segmented-control-item>
      </nldd-segmented-control>
    </div>

    <nldd-form-field
      :label="modus === 'doel' ? 'Beschrijf het beleidsdoel' : 'Geef een instructie'"
      :supporting-label="modusUitleg"
    >
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

    <!-- Twee assen: de regeling in miljoenen tegenover de uitvoeringslast, die
         een orde kleiner is. Op één as zou die laatste een platte lijn zijn,
         terwijl een doel als "gelijktrekken zonder dat de uitgave stijgt" juist
         over de verhouding tussen die twee gaat. -->
    <optimalisatiepad-chart
      v-if="modus === 'doel' && pad.length"
      :pad="pad"
      :reeksen="reeksen"
      :as-titels="['regeling', 'uitvoering']"
      :as-formatters="[euroCompact, euroCompact]"
      :gekozen="gekozenPunt"
      @kies="kiesPunt"
    />

    <div v-if="gekozenStand" class="as-punt">
      <div class="as-punt-kop">
        <strong>Meting {{ gekozenStand.iteratie }}</strong>
        <span>{{ gekozenStand.samenvatting }}</span>
        <nldd-icon-button icon="remove" size="sm" accessible-label="Sluit deze meting" @click="gekozenPunt = null"></nldd-icon-button>
      </div>
      <ul v-if="gekozenStand.wijzigingen?.length" class="as-punt-lijst">
        <li v-for="w in gekozenStand.wijzigingen" :key="`${w.artikel}:${w.naam}`">
          art. {{ w.artikel }} · <code>{{ w.naam }}</code>: {{ w.oud }} → {{ w.nieuw }}
        </li>
      </ul>
      <p v-else class="as-hint">Op deze meting was er nog niets aan de regeling gewijzigd.</p>
      <p v-if="gekozenStand.handelingenGewijzigd" class="as-hint">
        Het uitvoeringslastmodel was op dat moment ook bijgesteld; dat gaat niet mee met "overnemen".
      </p>
      <p v-if="gekozenStand.structuurGewijzigd?.length" class="as-hint">
        Ook de structuur van {{ gekozenStand.structuurGewijzigd.length === 1 ? 'een document' : `${gekozenStand.structuurGewijzigd.length} documenten` }}
        is herschreven; die wijziging staat niet als losse waarden in deze lijst.
      </p>
      <nldd-button
        v-if="gekozenStand.wijzigingen?.length"
        size="sm"
        variant="secondary"
        start-icon="save"
        :text="`Neem deze stand over in ${werkversieLabel}`"
        @click="neemStandOver(gekozenStand)"
      ></nldd-button>
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
import OptimalisatiepadChart from '@regelrecht/frontend-shared/components/OptimalisatiepadChart.vue';

// Geen `metrics`-emit meer. De assistent meet met zijn eigen n op zijn eigen
// tussenstand; die cijfers naast de tegels zetten zou de doorrekening van de
// gebruiker stil overschrijven met een tussenmeting. Ze horen thuis in de feed
// en in het optimalisatiepad.
const { streaming, run, abort } = useAssistent();

/**
 * De drie posten die een doel in deze casus tegen elkaar afweegt. Kleuren uit
 * de categorie-tokens van het ontwerpsysteem; echarts kent geen CSS-variabelen,
 * dus ze worden hier opgelost.
 */
function kleurVan(naam, terugval) {
  if (typeof document === 'undefined') return terugval;
  const el = document.createElement('span');
  el.style.cssText = `position:absolute;visibility:hidden;color:var(--semantics-categories-${naam}-filled-background-color)`;
  document.body.appendChild(el);
  const kleur = getComputedStyle(el).color;
  el.remove();
  return kleur && kleur !== 'rgba(0, 0, 0, 0)' ? kleur : terugval;
}

const reeksen = [
  { sleutel: 'regeling_po', label: 'regeling po', kleur: kleurVan('lintblauw', '#154273'), as: 0, formatter: euroCompact },
  { sleutel: 'regeling_vo', label: 'regeling vo', kleur: kleurVan('oranje', '#e17000'), as: 0, formatter: euroCompact },
  { sleutel: 'uitvoeringslast', label: 'uitvoeringslast', kleur: kleurVan('groen', '#39870c'), as: 1, formatter: euroCompact },
];

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
const {
  applyOverlays, applyDefinitionChange, docPathVoorKey,
  lawDocsFor, werkversie, werkversieLabel,
} = useLawStore();
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

const modus = ref('doel');
const prompt = ref('');
const feed = ref([]);
const pad = ref([]); // [{iteratie, waarden, …}] voor de doel-modus
const gekozenPunt = ref(null); // index in `pad`, of null
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

/**
 * Wat de twee modi van elkaar onderscheidt, in één regel. Zonder dit is het
 * verschil alleen uit het gedrag af te leiden: doel rekent iteratief naar een
 * uitkomst toe en mag daar zestig beurten over doen, instructie voert één
 * wijziging uit en laat het effect zien.
 */
const modusUitleg = computed(() =>
  modus.value === 'doel'
    ? 'Je weet wat je wilt bereiken. De assistent probeert wijzigingen, meet het effect en stelt bij tot het niet verder verbetert.'
    : 'Je weet wat je wilt veranderen. De assistent voert die ene wijziging uit en rekent door.',
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

const gekozenStand = computed(() => (gekozenPunt.value === null ? null : pad.value[gekozenPunt.value] ?? null));

function kiesPunt(index) {
  gekozenPunt.value = gekozenPunt.value === index ? null : index;
}

/**
 * Neem de stand van één meting over in de werkversie. Dat is niet de eindstand
 * van de assistent maar een tussenstap, en dat is precies waar dit voor is: een
 * pad loopt soms door een uitkomst heen die je beter bevalt dan waar hij
 * uitkomt.
 */
function neemStandOver(stand) {
  const mislukt = [];
  for (const w of stand.wijzigingen) {
    const docPad = docPathVoorKey(w.document_key);
    if (!docPad) { mislukt.push(`${w.naam} (document ${w.document_key} niet gevonden)`); continue; }
    try {
      applyDefinitionChange(docPad, w.artikel, w.naam, w.nieuw);
    } catch (e) {
      mislukt.push(`${w.naam} (${e?.message ?? e})`);
    }
  }
  const aantal = stand.wijzigingen.length - mislukt.length;
  feed.value.push({
    type: mislukt.length ? 'fout' : 'tekst',
    tekst: `Meting ${stand.iteratie} overgenomen in ${werkversieLabel.value}: ${aantal} van de ${stand.wijzigingen.length} wijzigingen.`,
    melding: mislukt.length ? `Niet overgenomen: ${mislukt.join('; ')}.` : undefined,
  });
  gekozenPunt.value = null;
}

async function submit() {
  feed.value = [];
  pad.value = [];
  gekozenPunt.value = null;
  overlays.value = null;
  handelingenYaml.value = null;
  resultaat.value = false;
  let iteratie = 0;

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
      const kort = samenvatting(ev.metrics);
      feed.value.push({ type: 'simulatie', doel: ev.doel, n: ev.n, samenvatting: kort });
      const t = ev.metrics?.totaal;
      if (ev.doel === 'populatie' && t) {
        pad.value.push({
          iteratie: ++iteratie,
          waarden: {
            regeling_po: t.regeling_po ?? null,
            regeling_vo: t.regeling_vo ?? null,
            uitvoeringslast: t.uitvoeringslast ?? null,
          },
          samenvatting: kort,
          n: ev.n ?? null,
          wijzigingen: ev.wijzigingen ?? [],
          structuurGewijzigd: ev.structuurGewijzigd ?? [],
          handelingenGewijzigd: !!ev.handelingenGewijzigd,
        });
      }
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
.as-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.as-punt {
  display: flex; flex-direction: column; gap: var(--primitives-space-8);
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  border: 1px solid var(--semantics-dividers-color);
}
.as-punt-kop { display: flex; align-items: center; gap: var(--primitives-space-8); font-size: 0.9em; }
.as-punt-kop nldd-icon-button { margin-left: auto; }
.as-punt-lijst { margin: 0; padding-left: 1.2em; font-size: 0.85em; display: flex; flex-direction: column; gap: 2px; }
</style>
