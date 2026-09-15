<template>
  <div class="bn">
    <p class="bn-hint">
      Zoek het bedrag waarbij de werkversie evenveel uitgeeft als huidig recht.
      Elke iteratie is een volledige doorrekening van de werkversiekolom.
    </p>

    <nldd-banner v-if="!werkversie" variant="accent">
      De werkversie is nu huidig recht. Kies een variant als werkversie (balk bovenaan) om daarin een budgetneutrale
      waarde te zoeken.
      <span v-if="changeCount === 0 && variantColumns.length" class="bn-row">
        <nldd-button v-for="c in variantColumns" :key="c.key" size="sm" variant="secondary" :text="`Werk in ${c.short}`" @click="setWerkversie(c.variantId)"></nldd-button>
      </span>
    </nldd-banner>

    <template v-else>
      <nldd-form-field label="Parameter (bedrag)" :supporting-label="`definitie in eurocent uit ${werkversieLabel}`">
        <nldd-dropdown :key="`${werkversie}:${definities.length}:${defKey}`" size="sm" @change="defKey = $event.detail?.value ?? defKey">
          <select :value="defKey">
            <option v-for="d in definities" :key="d.key" :value="d.key">{{ d.label }}</option>
          </select>
        </nldd-dropdown>
      </nldd-form-field>

      <nldd-form-field label="Gelijk houden">
        <nldd-segmented-control size="sm" :value="doelpost" @change="doelpost = $event.detail?.value ?? doelpost">
          <nldd-segmented-control-item value="regeling_po" text="Regeling po"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="regeling_vo" text="Regeling vo"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="regeling_totaal" text="Totaal"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-form-field>

      <div class="bn-row">
        <nldd-button
          :text="solving ? 'Zoeken…' : 'Zoek budgetneutrale waarde'"
          start-icon="graph"
          variant="secondary"
          :disabled="solving || !defKey || !istMetrics ? true : undefined"
          @click="solve"
        ></nldd-button>
        <nldd-activity-indicator v-if="solving" size="20" timing="instant"></nldd-activity-indicator>
      </div>

      <nldd-banner v-if="error" variant="critical">{{ error }}</nldd-banner>

      <div v-if="solveLog.length" class="bn-log">
        <div v-for="(step, i) in solveLog" :key="i" class="bn-step">
          <span class="bn-i">{{ i + 1 }}</span>
          <span>{{ euro(step.value) }}</span>
          <nldd-icon name="arrow-right" size="16"></nldd-icon>
          <span>{{ euroCompact(step.spend) }}</span>
        </div>
      </div>

      <nldd-inline-dialog v-if="result" :variant="result.converged === false ? 'alert' : 'success'" :text="resultTekst">
        <nldd-button
          slot="actions"
          :text="`Toepassen in ${werkversieLabel}`"
          size="sm"
          variant="primary"
          @click="apply"
        ></nldd-button>
      <p v-if="applied" class="bn-hint">{{ applied }}</p>
      </nldd-inline-dialog>
    </template>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue';
import { useSimulation } from '../../composables/useSimulation.js';
import { useLawStore } from '../../engine/lawStore.js';
import { listDefinitions } from '../../lib/yamlPatch.js';
import { euro, euroCompact } from '../../lib/format.js';
import { LAW_PO } from '../../lib/nieuwkomerFacts.js';

const { columns, istMetrics, solving, solveLog, solveBudgetneutraal } = useSimulation();
const { lawDocsFor, docPaths, editableDocs, applyDefinitionChange, werkversie, werkversieLabel, changeCount, setWerkversie, version } = useLawStore();

const defKey = ref(null);
const doelpost = ref('regeling_po');
const definities = ref([]);
const result = ref(null);
const error = ref(null);
const applied = ref(null);

const variantColumns = computed(() => columns.value.filter((c) => c.variantId));

// De definities van de werkversie; opnieuw als de werkversie wisselt of als
// een document verandert (bijvoorbeeld na Toepassen: dan klopt het bedrag in
// het label weer).
watch([werkversie, version], async ([id]) => {
  const vorige = defKey.value;
  definities.value = [];
  defKey.value = null;
  if (!id) { result.value = null; applied.value = null; return; }
  // De documenten van de werkversie (inclusief het nieuwe versiebestand van de
  // variant), alleen bewerkbare wetten en per wet alleen de nieuwste versie:
  // aan een oudere versie draaien verandert hooguit één jaar.
  const kolomDocs = await lawDocsFor(id);
  const editableIds = new Set(editableDocs.value.map((d) => d.entry.id));
  const nieuwste = new Map();
  for (const d of kolomDocs) {
    if (!editableIds.has(d.entry.id)) continue;
    const huidige = nieuwste.get(d.entry.id);
    if (!huidige || String(d.entry.valid_from) > String(huidige.entry.valid_from)) nieuwste.set(d.entry.id, d);
  }
  const volgorde = [...nieuwste.values()].sort((a, b) => (a.entry.id === LAW_PO ? -1 : b.entry.id === LAW_PO ? 1 : 0));
  const list = [];
  for (const d of volgorde) {
    const versie = String(d.entry.valid_from ?? '').slice(0, 4);
    for (const def of listDefinitions(d.yaml)) {
      if (def.unit !== 'eurocent') continue;
      // Nulconstanten (geen_bedrag, geen_toeslag) zijn geen knoppen.
      if (!(Number(def.value) > 0)) continue;
      list.push({
        key: `${d.path}::${def.article}::${def.name}`,
        path: d.path,
        article: def.article,
        name: def.name,
        label: `art. ${def.article} · ${def.name} (${euro(def.value)}) · ${(d.entry.name ?? d.path).slice(0, 36)} ${versie}`,
      });
    }
  }
  definities.value = list;
  // Zelfde keuze houden als die nog bestaat; anders standaard een tarief per
  // leerling, niet een eenmalige toeslag.
  defKey.value = (list.find((d) => d.key === vorige) ?? list.find((d) => d.name.startsWith('bedrag_per_')) ?? list[0])?.key ?? null;
}, { immediate: true });

const gekozen = computed(() => definities.value.find((d) => d.key === defKey.value) ?? null);

const resultTekst = computed(() => {
  const r = result.value;
  if (!r) return '';
  const status = r.converged === false ? 'niet geconvergeerd na' : 'gevonden in';
  return `${gekozen.value?.name} = ${euro(r.value)} geeft ${euroCompact(r.spend)} (doel ${euroCompact(r.target)}), ${status} ${r.iterations} iteraties.`;
});

async function solve() {
  const d = gekozen.value;
  if (!d) return;
  error.value = null;
  result.value = null;
  try {
    result.value = await solveBudgetneutraal({
      variantId: werkversie.value,
      lawPath: d.path,
      article: d.article,
      name: d.name,
      doelpost: doelpost.value,
    });
  } catch (e) {
    error.value = String(e?.message ?? e);
  }
}

async function apply() {
  const d = gekozen.value;
  if (!d || !result.value) return;
  error.value = null;
  applied.value = null;
  try {
    // De werkversie staat in de store; de waarde wordt daar een bewerking.
    if (!docPaths.value.includes(d.path)) throw new Error(`Document ${d.path} niet gevonden in de werkversie`);
    applyDefinitionChange(d.path, d.article, d.name, result.value.value);
    applied.value = `${d.name} = ${euro(result.value.value)} gezet als bewerking in ${werkversieLabel.value}; de kolommen rekenen opnieuw. Terugdraaien: Terugzetten; vastleggen: Bewaar als variant.`;
  } catch (e) {
    error.value = String(e?.message ?? e);
  }
}
</script>

<style scoped>
.bn { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.bn-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.bn-row { display: flex; align-items: center; gap: var(--primitives-space-8); }
.bn-log {
  display: flex; flex-direction: column; gap: 2px;
  font-size: 0.85em; font-variant-numeric: tabular-nums;
  padding: var(--primitives-space-8) var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  border: 1px solid var(--semantics-dividers-color);
}
.bn-step { display: flex; align-items: center; gap: 6px; }
.bn-i { width: 1.5em; color: var(--semantics-content-secondary-color); }
</style>
