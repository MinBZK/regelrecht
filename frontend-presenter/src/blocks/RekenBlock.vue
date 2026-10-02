<script setup>
import { computed, inject, ref, watch } from 'vue';
import { humanize } from '@editor-utils/outputFormat.js';
import { deckEngine } from '../engine/engine.js';
import { errorText, runScenario } from '../engine/runScenario.js';
import { formatInUnit } from '../lib/format.js';
import TraceNode from './TraceNode.vue';

// A ```reken block: one scenario from a .feature file, run by the engine on
// the slide.
//   scenario: wet_op_de_zorgtoeslag/eligibility.feature   (next to the law)
//     or:     mijn-casus.feature                           (in the deck folder)
//   naam: Laag inkomen alleenstaande heeft recht op zorgtoeslag
//   uitvoer: [hoogte_zorgtoeslag]    optional: which outputs to show
//   spoor: true                      optional: the calculation trace
//
// The case is the scenario's own data, so what the slide shows is the case
// that CI tests, and its Then steps show whether the outcome is the expected one.

const props = defineProps({ spec: { type: Object, required: true } });
const deckCtx = inject('presenter:deck', null);

const state = ref({ status: 'idle' });
// Only the newest run may set the state: a slow older run (a law still
// loading) must not overwrite the answer to a newer spec.
let runId = 0;

async function run() {
  const id = ++runId;
  const deck = deckCtx?.name.value;
  const set = (v) => {
    if (id === runId) state.value = v;
  };
  set({ status: 'running' });
  try {
    const q = new URLSearchParams({ feature: String(props.spec.scenario), deck: deck ?? '' });
    const res = await fetch(`/api/scenario?${q}`);
    const feature = await res.json();
    if (!res.ok) throw new Error(feature.error);
    const ctx = await deckEngine(deck);
    const result = await ctx.exclusive(() =>
      runScenario({ engine: ctx.engine, text: feature.text, name: props.spec.naam, loadLaw: ctx.loadLaw }),
    );
    set({ status: 'done', result, units: ctx.units, failures: [...ctx.failures] });
  } catch (e) {
    set({ status: 'error', message: errorText(e) });
  }
}

watch(() => [JSON.stringify(props.spec), deckCtx?.name.value, deckCtx?.wetVersion.value], run, { immediate: true });

const MARKS = { passed: '✓', failed: '✗', unsupported: '·', skipped: '–' };
const result = computed(() => state.value.result);
const wip = computed(() => result.value?.scenario.tags.includes('@wip'));
const checks = computed(() => (result.value?.steps ?? []).filter((s) => s.kind === 'Then'));
const failedStep = computed(() => result.value?.steps.find((s) => s.status === 'failed' && s.kind !== 'Then'));

/** The outputs to show: the ones asked for, else the ones evaluated and asserted. */
const outputs = computed(() => {
  const r = result.value?.run;
  if (!r) return [];
  const asserted = checks.value.map((s) => s.text.match(/output "([^"]+)"/)?.[1]).filter(Boolean);
  const names = props.spec.uitvoer != null ? [].concat(props.spec.uitvoer).map(String) : [...new Set([...r.asked, ...asserted])];
  return names.map((name) => ({
    name,
    label: humanize(name),
    present: name in r.outputs,
    value: formatInUnit(r.outputs[name], state.value.units?.get(`${r.law}/${name}`)),
  }));
});

const lawName = computed(() => {
  const id = result.value?.run?.law;
  if (!id) return '';
  const n = humanize(id);
  return n.charAt(0).toUpperCase() + n.slice(1);
});
</script>

<template>
  <figure class="card reken">
    <figcaption class="wet-head">
      <span class="wet-law">{{ result?.scenario.name ?? spec.naam ?? 'Berekening' }}</span>
      <span v-if="lawName" class="wet-art">{{ lawName }}</span>
      <span v-if="wip" class="wet-date">@wip: nog niet de gewenste uitkomst</span>
    </figcaption>

    <p v-if="state.status === 'running' || state.status === 'idle'" class="wet-loading">Rekenen…</p>
    <p v-else-if="state.status === 'error'" class="wet-error">{{ state.message }}</p>
    <template v-else>
      <!-- Folded: the engine holds the whole corpus, so a refused law may be
           one this scenario never touches. It is there for when the outcome
           looks wrong. -->
      <details v-if="state.failures.length" class="wet-warnings">
        <summary>De engine weigerde {{ state.failures.length }} wetversie(s)</summary>
        <ul>
          <li v-for="f in state.failures" :key="f.law + f.source">{{ f.law }} ({{ f.source }}): {{ f.message }}</li>
        </ul>
      </details>

      <table v-if="outputs.length" class="reken-outputs">
        <tr v-for="o in outputs" :key="o.name">
          <th>{{ o.label }}</th>
          <td class="num">{{ o.present ? o.value : 'niet berekend' }}</td>
        </tr>
      </table>
      <p v-if="result.error" class="wet-error">De engine gaf een fout: {{ result.error }}</p>
      <p v-if="failedStep" class="wet-error">{{ failedStep.keyword }} {{ failedStep.text }}: {{ failedStep.message }}</p>

      <section v-if="checks.length">
        <h4>Verwacht volgens het scenario</h4>
        <ul class="reken-checks">
          <li v-for="(c, i) in checks" :key="i" :class="c.status">
            <span class="reken-mark" aria-hidden="true">{{ MARKS[c.status] ?? '–' }}</span>
            {{ c.text }}
            <span v-if="c.message" class="reken-msg">{{ c.message }}</span>
          </li>
        </ul>
      </section>

      <section v-if="spec.spoor && result.run?.trace" class="reken-trace">
        <h4>Rekenspoor</h4>
        <TraceNode :node="result.run.trace" />
      </section>
    </template>
  </figure>
</template>
