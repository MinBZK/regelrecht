<template>
  <nldd-side-by-side-split-view panes="2">
    <!-- LINKS: wat vergelijk je, wat wijzig je, hulpmiddelen, instellingen -->
    <div slot="pane-1" class="pane pane-left">
      <nldd-page>
        <div class="pane-inner">
          <nldd-title :size="2">
            <span slot="overline">Voor beleidsmakers</span>
            <span>Nieuwkomersbekostiging: regeling en uitvoering</span>
          </nldd-title>

          <nldd-banner v-if="mountError && !initError" variant="critical">
            Doorrekenen mislukt: {{ mountError.message }}
          </nldd-banner>
          <nldd-banner v-if="initError" variant="critical">
            De rekenmachine kon niet worden geladen: {{ initError.message }}
          </nldd-banner>
          <nldd-banner v-else-if="!ready" variant="accent">Rekenmachine wordt geladen…</nldd-banner>

          <template v-if="ready">
            <paneel titel="Kolommen" subtitel="Huidig recht staat altijd; kies tot drie varianten ernaast." :samenvatting="kolommenSamenvatting" open>
              <variant-switcher multiple :model-value="selectedVariants" :max="3" @update:model-value="selectedVariants = $event" />
            </paneel>

            <paneel :titel="`Regeling bijstellen`" :subtitel="`Bedragen, duur en drempel van ${werkversieLabel}. Elke wijziging rekent de werkversiekolom opnieuw.`" :badge="hasChanges ? `${changeCount} bewerkt` : ''" open>
              <parameter-panel />
              <details class="geavanceerd">
                <summary>Geavanceerd: YAML rechtstreeks bewerken</summary>
                <div class="geav-row">
                  <nldd-dropdown :key="`ye:${editableDocs.length}:${yamlKeuze}`" size="sm" width="420px" @change="yamlKeuze = $event.detail?.value ?? yamlKeuze">
                    <select :value="yamlKeuze ?? ''" aria-label="Document om als YAML te bewerken">
                      <option v-for="d in editableDocs" :key="d.entry.path" :value="d.entry.path">{{ d.entry.name }} (geldig vanaf {{ String(d.entry.valid_from ?? '').slice(0, 4) }})</option>
                    </select>
                  </nldd-dropdown>
                  <nldd-button text="Open in YAML-editor" start-icon="code" variant="secondary" size="sm" :disabled="!yamlKeuze ? true : undefined" @click="openYaml(yamlKeuze)"></nldd-button>
                </div>
              </details>
            </paneel>

            <paneel titel="Budgetneutraal" subtitel="Wat kan er veranderen zonder dat het meer geld kost? Zoekt het bedrag waarbij de werkversie evenveel uitgeeft als huidig recht." samenvatting="zoekt een bedrag">
              <budgetneutraal-panel />
            </paneel>

            <paneel titel="Beleidsassistent" subtitel="Een instructie of doel in gewone taal; de assistent wijzigt de werkversie en rekent door." samenvatting="instructie of doel">
              <assistent-panel />
            </paneel>

            <paneel titel="Uitvoeringslastmodel" subtitel="Handelingen × minuten × tarief per partij, investeringen en budget. Bijstellen is direct zichtbaar." :samenvatting="uitvoeringSamenvatting" :badge="hasOverrides ? 'aangepast' : ''">
              <handelingen-panel />
            </paneel>

            <paneel titel="Populatie" subtitel="Eén vaste steekproef van synthetische leerlingen, gewogen naar de echte instroom. Alle kolommen rekenen met dezelfde leerlingen." :samenvatting="populatieSamenvatting" :badge="popGewijzigd ? 'aangepast' : ''">
              <population-controls />
            </paneel>
          </template>
        </div>
      </nldd-page>
    </div>

    <!-- RECHTS: de twee rekeningen -->
    <div slot="pane-2" class="pane pane-right">
      <nldd-page background="tinted">
        <div class="pane-inner">
          <div class="kop-rij">
            <nldd-title :size="3">
              <span>Twee rekeningen naast elkaar</span>
              <span slot="subtitle">Wat de regeling kost (bedragen aan scholen) en wat de uitvoering kost (handelingen bij scholen en DUO, plus investering).</span>
            </nldd-title>
            <doorreken-knop v-if="ready" class="kop-actie" />
          </div>

          <nldd-banner v-if="fouten" variant="warning">
            {{ fouten }}
          </nldd-banner>
          <nldd-banner v-if="nietInWerking" variant="accent">
            {{ nietInWerking }}
          </nldd-banner>

          <template v-if="ready">
            <section class="block">
              <kolom-tabel :columns="columns" :metrics-by-column="metricsByColumn" :jaren="jaren" :running="running" :progress="progress" />
            </section>

            <section class="block charts">
              <uitgaven-chart
                title="Regeling per jaar"
                subtitle="bedragen aan scholen"
                :columns="columns"
                :metrics-by-column="metricsByColumn"
                :jaren="jaren"
                :posten="REGELING_POSTEN"
              />
              <uitgaven-chart
                title="Uitvoering per jaar"
                subtitle="uitvoeringslast per partij en investering"
                :columns="columns"
                :metrics-by-column="metricsByColumn"
                :jaren="jaren"
                :posten="UITVOERING_POSTEN"
              />
            </section>

            <paneel titel="Huidig recht in cijfers" samenvatting="kengetallen per jaar">
              <metric-tiles :metrics="istMetrics" />
            </paneel>

            <paneel titel="Plausibiliteit" samenvatting="simulatie naast factsheet en realisatie">
              <plausibiliteit-tabel :metrics="istMetrics" :distributions="distributions" />
            </paneel>
          </template>
        </div>
      </nldd-page>
    </div>
  </nldd-side-by-side-split-view>

  <!-- Sheets buiten de split-view: daarbinnen zou de default-slot niet gerenderd worden. -->
  <yaml-editor-sheet :open="yamlOpen" :law-path="yamlPath" @close="yamlOpen = false" />
</template>

<script setup>
import { ref, computed, watch, onMounted } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';
import { useSimulation } from '../composables/useSimulation.js';
import { useHandelingen } from '../composables/useHandelingen.js';
import { number } from '../lib/format.js';
import Paneel from '@regelrecht/frontend-shared/components/Paneel.vue';
import VariantSwitcher from '../components/VariantSwitcher.vue';
import ParameterPanel from '../components/beleid/ParameterPanel.vue';
import HandelingenPanel from '../components/beleid/HandelingenPanel.vue';
import BudgetneutraalPanel from '../components/beleid/BudgetneutraalPanel.vue';
import PopulationControls from '../components/beleid/PopulationControls.vue';
import { usePopulatie } from '../composables/usePopulatie.js';
import DoorrekenKnop from '../components/beleid/DoorrekenKnop.vue';
import KolomTabel from '../components/beleid/KolomTabel.vue';
import UitgavenChart from '../components/beleid/UitgavenChart.vue';
import MetricTiles from '../components/beleid/MetricTiles.vue';
import PlausibiliteitTabel from '../components/beleid/PlausibiliteitTabel.vue';
import YamlEditorSheet from '../components/beleid/YamlEditorSheet.vue';
import AssistentPanel from '../components/beleid/AssistentPanel.vue';

const { ready, initError, lawIndex, initEngine } = useEngine();
const { initStore, hasChanges, changeCount, editableDocs, werkversieLabel } = useLawStore();
const { selectedVariants, columns, metricsByColumn, istMetrics, jaren, simVersion, distributions, recompute, running, progress, n } = useSimulation();
const { fetchHandelingen, base, overrides, hasOverrides, alleHandelingen } = useHandelingen();
const { instroomRijen, instroomTotaal, hasOverrides: popGewijzigd } = usePopulatie();

const REGELING_POSTEN = [
  { key: 'po', label: 'Regeling po', pick: (m) => m.regeling_po.totaal },
  { key: 'vo', label: 'Regeling vo', pick: (m) => m.regeling_vo.totaal },
];
const UITVOERING_POSTEN = [
  { key: 'school', label: 'Uitvoeringslast school', pick: (m) => m.uitvoeringslast.school },
  { key: 'duo', label: 'Uitvoeringslast DUO', pick: (m) => m.uitvoeringslast.duo },
  { key: 'overig', label: 'Uitvoeringslast overig', pick: (m) => m.uitvoeringslast.overig },
  { key: 'inv', label: 'Investering (afschrijving)', pick: (m) => m.investering },
];

const mountError = ref(null);
const yamlOpen = ref(false);
const yamlPath = ref(null);
const yamlKeuze = ref(null);

// Standaard het nieuwste bewerkbare document in de YAML-keuzelijst.
watch(editableDocs, (docs) => {
  if (!docs.some((d) => d.entry.path === yamlKeuze.value)) yamlKeuze.value = docs[docs.length - 1]?.entry.path ?? null;
}, { immediate: true });

// Samenvattingen op de dichtgeklapte panelen: de huidige stand in één regel.
const kolommenSamenvatting = computed(() => (selectedVariants.value.length ? `huidig recht + ${selectedVariants.value.length} variant${selectedVariants.value.length === 1 ? '' : 'en'}` : 'alleen huidig recht'));
const populatieSamenvatting = computed(() => {
  const instroom = instroomTotaal.value
    ? `${number(instroomTotaal.value)} nieuwkomers over ${instroomRijen.value.length} instroomjaren`
    : 'instroom uit CBS en de OCW-factsheet';
  return `${instroom} · ${number(n.value)} records · in elke kolom dezelfde leerlingen`;
});
const uitvoeringSamenvatting = computed(() => {
  const b = base.value?.budget ?? {};
  const mln = (key) => Math.round(((overrides.budget?.[key] ?? b[key] ?? 0) / 1e8) * 10) / 10;
  return `${alleHandelingen.value.length} handelingen · budget po ${mln('regeling_po')} / vo ${mln('regeling_vo')} / uitvoering ${mln('uitvoering')} mln`;
});

const fouten = computed(() => {
  const m = istMetrics.value;
  if (!m?.fouten?.aantal) return null;
  return `De engine gaf ${m.fouten.aantal} fouten bij huidig recht (corpus in aanbouw?). Eerste: ${m.fouten.voorbeelden[0]}`;
});

const nietInWerking = computed(() => {
  const m = istMetrics.value;
  if (!m?.niet_in_werking) return null;
  const delen = [];
  for (const [sector, lijst] of Object.entries(m.niet_in_werking)) {
    if (lijst.length) delen.push(`${sector}: geen regeling in werking op ${lijst.length} peildata vanaf ${lijst[0]} (uitgaven daar 0)`);
  }
  return delen.length ? delen.join(' · ') : null;
});

function openYaml(path) {
  if (!path) return;
  yamlPath.value = path;
  yamlOpen.value = true;
}

onMounted(async () => {
  try {
    const engine = await initEngine();
    await initStore(engine, lawIndex.value);
    await fetchHandelingen();
    if (simVersion.value === -1) await recompute();
  } catch (e) {
    // initError toont engine-fouten; alles daarna hoort hier zichtbaar te zijn.
    console.error('Beleidsview initialisatie mislukt', e);
    mountError.value = e;
  }
});
</script>

<style scoped>
.pane { height: 100%; }
.pane-inner {
  padding: var(--primitives-space-24);
  display: flex;
  flex-direction: column;
  gap: var(--primitives-space-16);
}
.block { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.charts { display: grid; grid-template-columns: repeat(auto-fit, minmax(360px, 1fr)); gap: var(--primitives-space-16); }
.kop-rij { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--primitives-space-16); flex-wrap: wrap; }
.kop-actie { flex: 0 0 auto; }
.geavanceerd summary { cursor: pointer; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.geav-row { display: flex; gap: var(--primitives-space-8); align-items: center; flex-wrap: wrap; margin-top: var(--primitives-space-8); }
</style>
