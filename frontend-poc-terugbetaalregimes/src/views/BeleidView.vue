<template>
  <nldd-side-by-side-split-view panes="2">
    <!-- LINKS: wat je wijzigt, hulpmiddelen, instellingen -->
    <div slot="pane-1" class="pane pane-left">
      <nldd-page>
        <div class="pane-inner">
          <nldd-title :size="2">
            <span slot="overline">Voor beleidsmakers</span>
            <span>Terugbetaalregimes: draaien aan de wet</span>
          </nldd-title>

          <nldd-banner v-if="initError" variant="critical">
            De rekenmachine kon niet worden geladen: {{ initError.message }}
          </nldd-banner>
          <nldd-banner v-else-if="!ready" variant="accent">Rekenmachine wordt geladen…</nldd-banner>

          <template v-if="ready">
            <paneel titel="Wet bijstellen" :subtitel="`Percentages, voeten en termijnen van ${werkversieLabel}. Elke wijziging rekent de werkversie opnieuw door.`" :samenvatting="bijstellenSamenvatting" :badge="hasChanges ? `${changeCount} bewerkt` : ''">
              <parameter-panel />
              <details class="geavanceerd">
                <summary>Geavanceerd: YAML rechtstreeks bewerken</summary>
                <div class="geav-row">
                  <nldd-button
                    v-for="d in editableDocs"
                    :key="d.entry.path"
                    :text="`Open ${d.entry.name} in de YAML-editor`"
                    start-icon="code"
                    variant="secondary"
                    size="sm"
                    @click="openYaml(d.entry.path)"
                  ></nldd-button>
                </div>
              </details>
            </paneel>

            <paneel titel="Beleidsassistent" subtitel="Een instructie of doel in gewone taal; de assistent wijzigt de werkversie en rekent door." samenvatting="instructie of doel">
              <assistent-panel />
            </paneel>

            <paneel titel="Uitvoeringslastmodel" subtitel="Handelingen × minuten × tarief. Wat DUO het kost staat in euro's, wat het debiteuren kost in uren." :samenvatting="uitvoeringSamenvatting">
              <handelingen-panel />
            </paneel>

            <paneel titel="Populatie" subtitel="Eén vaste steekproef van synthetische debiteuren, gewogen naar de echte aantallen. Beide kolommen rekenen met dezelfde debiteuren." :samenvatting="populatieSamenvatting">
              <population-controls />
            </paneel>

            <paneel titel="Varianten uit de Stand van de Uitvoering" subtitel="De vier beleidsvarianten als branches, met de kostenraming uit de brief." :samenvatting="`${variants.length} varianten · werkversie ${werkversieLabel}`">
              <varianten-lijst />
            </paneel>
          </template>
        </div>
      </nldd-page>
    </div>

    <!-- RECHTS: effecten, huidig recht naast de werkversie -->
    <div slot="pane-2" class="pane pane-right">
      <nldd-page background="tinted">
        <div class="pane-inner">
          <div class="kop-rij">
            <nldd-title :size="3">
              <span>{{ werkversie || hasChanges ? `Effecten: huidig recht naast ${kolomLabel}` : 'Effecten onder huidig recht' }}</span>
              <span slot="subtitle">{{ hasChanges ? `${werkversieLabel} met ${changeCount} bewerkt${changeCount === 1 ? '' : 'e'} document${changeCount === 1 ? '' : 'en'}; de pijlen en de grafiek vergelijken met huidig recht.` : werkversie ? 'De variant zoals hij op de branch staat, vergeleken met huidig recht.' : 'Wijzig links een parameter of kies een variant als werkversie; dan komt huidig recht ernaast te staan.' }}</span>
            </nldd-title>
            <doorreken-knop v-if="ready" class="kop-actie" />
          </div>

          <template v-if="ready">
            <section class="block">
              <nldd-title :size="4">
                <span>De posten per kolom</span>
                <span slot="subtitle">Wat de regeling doet met de OCW-begroting, wat debiteuren ervan merken en welke volumes bij DUO landen. Vink links varianten aan om ze als kolom toe te voegen.</span>
              </nldd-title>
              <kolom-tabel :columns="columns" :metrics-by-column="metricsByColumn" :running="running" />
            </section>

            <section class="block">
              <nldd-title :size="4">
                <span>Populatie</span>
                <span slot="subtitle">De tegels zijn kengetallen onder {{ kolomLabel }}. De grafiek eronder zet dezelfde kolommen als de tabel naast elkaar, per terugbetaalregime.</span>
              </nldd-title>
              <metric-tiles :metrics="metrics" :baseline-metrics="baselineMetrics" :kolom-label="kolomLabel" />
              <regime-metrics-chart :columns="columns" :metrics-by-column="metricsByColumn" />
            </section>
          </template>
        </div>
      </nldd-page>
    </div>
  </nldd-side-by-side-split-view>

  <!-- Sheets buiten de split-view: daarbinnen zou de default-slot niet gerenderd worden. -->
  <yaml-editor-sheet :open="yamlOpen" :law-path="yamlPath" @close="yamlOpen = false" />
</template>

<script setup>
import { ref, computed, onMounted } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';
import { usePersonas } from '../composables/usePersonas.js';
import { usePopulation } from '../composables/usePopulation.js';
import { usePopulatieAannames } from '../composables/usePopulatieAannames.js';
import { number, euroCompact } from '../lib/format.js';
import Paneel from '@regelrecht/frontend-shared/components/Paneel.vue';
import ParameterPanel from '../components/beleid/ParameterPanel.vue';
import KolomTabel from '../components/beleid/KolomTabel.vue';
import MetricTiles from '../components/beleid/MetricTiles.vue';
import YamlEditorSheet from '../components/beleid/YamlEditorSheet.vue';
import PopulationControls from '../components/beleid/PopulationControls.vue';
import DoorrekenKnop from '../components/beleid/DoorrekenKnop.vue';
import RegimeMetricsChart from '../components/beleid/RegimeMetricsChart.vue';
import AssistentPanel from '../components/beleid/AssistentPanel.vue';
import HandelingenPanel from '../components/beleid/HandelingenPanel.vue';
import VariantenLijst from '../components/beleid/VariantenLijst.vue';

const { ready, initError, lawIndex, initEngine } = useEngine();
const { initStore, hasChanges, changeCount, editableDocs, werkversie, werkversieLabel, variants } = useLawStore();
const { fetchPersonas } = usePersonas();
const { metrics, baselineMetrics, columns, metricsByColumn, running, simVersion, recompute, n } = usePopulation();
const { totaalDebiteuren } = usePopulatieAannames();

const yamlOpen = ref(false);
const yamlPath = ref(null);

/** Naam van de werkversiekolom: de variant, of huidig recht (met bewerkingen). */
const kolomLabel = computed(() => (werkversie.value ? werkversieLabel.value : hasChanges.value ? 'huidig recht met bewerkingen' : 'huidig recht'));

// Dicht zegt het paneel waar het over gaat en of eraan gewerkt is; open zou het
// als eerste paneel de hele linkerkolom vullen voordat iemand iets gekozen heeft.
const bijstellenSamenvatting = computed(() => (hasChanges.value ? `${werkversieLabel.value} bewerkt` : `percentages en termijnen van ${werkversieLabel.value}`));

// Dicht toont het paneel de twee eenheden naast elkaar; dat verschil (euro's
// bij DUO, uren bij de debiteur) is waar dit model over gaat.
const uitvoeringSamenvatting = computed(() => {
  const last = metrics.value?.uitvoeringslast;
  if (!last) return 'nog niet doorgerekend';
  return `${euroCompact(last.kostenTotaal)} bij DUO · ${number(Math.round(last.urenBurger))} uur bij debiteuren`;
});
const populatieSamenvatting = computed(() => {
  const debiteuren = totaalDebiteuren.value ? `${number(totaalDebiteuren.value)} aflossende debiteuren` : 'debiteuren uit CBS en de Stand van DUO';
  return `${debiteuren} · ${number(n.value)} records · in elke kolom dezelfde debiteuren`;
});

function openYaml(path) {
  yamlPath.value = path;
  yamlOpen.value = true;
}

onMounted(async () => {
  await fetchPersonas().catch(() => {});
  try {
    const engine = await initEngine();
    await initStore(engine, lawIndex.value);
    if (simVersion.value === -1) await recompute();
  } catch (e) {
    // initError toont engine-fouten; simulatiefouten staan per kolom bij Doorrekenen.
    console.error('Beleidsview initialisatie mislukt', e);
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
.kop-rij { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--primitives-space-16); flex-wrap: wrap; }
.kop-actie { flex: 0 0 auto; }
.geavanceerd summary { cursor: pointer; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.geav-row { display: flex; gap: var(--primitives-space-8); align-items: center; flex-wrap: wrap; margin-top: var(--primitives-space-8); }
</style>
