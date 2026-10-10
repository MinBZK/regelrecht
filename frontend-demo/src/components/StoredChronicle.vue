<script setup>
import { computed, nextTick, reactive, watch } from 'vue';
import { formatDate, humanize } from '../data/format.js';
import { inputRows } from '../data/chronicleView.js';
import { cellsOfService } from '../data/chronolex.js';
import { fieldsByArticle, onlyCase, provenanceText, storedChronicle } from '../data/storedChronicle.js';
import { useDemo } from '../store/demoStore.js';
import { useI18n } from '../i18n/index.js';
import { useLocalePath } from '../i18n/useLocalePath.js';
import { useProvisionLinks } from '../useProvisionLinks.js';

// De kroniek vanaf de achterkant: elke gram die de cel van deze organisatie
// heeft opgeslagen, in de volgorde van vastleggen, met wat de wet in haar
// eigen woorden zegt dat er ontstaat (de vorm die de cel uit de wet afleidt)
// en de registratie in de stroom van de cel. Per gram ook de gram zelf, zoals
// de cel hem opslaat. Welke gebeurtenis of wet het is, staat in de grammen en
// de cel; deze component noemt er geen.

const props = defineProps({
  /** De organisatie waarvan de cellen getoond worden. */
  service: { type: String, default: null },
  /** Alleen de zaak die met deze gram begon; leeg = de hele kroniek. */
  root: { type: String, default: null },
  /** De gram die opengeklapt in beeld komt (een link vanuit een andere weergave). */
  focus: { type: String, default: null },
});
const emit = defineEmits(['clear-root']);

const { t } = useI18n();
const { goTo } = useLocalePath();
const demo = useDemo();
const { corpus, state, dataVersion } = demo;

/** De cellen die een wet van deze organisatie uitvoeren, elk één keer. */
const cells = computed(() => cellsOfService(corpus.value, props.service));

const chronicles = computed(() => {
  void dataVersion.value;
  return cells.value.map((cell) => {
    const all = storedChronicle(cell, state.grams);
    return { cell, total: all.length, entries: onlyCase(all, props.root).map(describe) };
  });
});

const caseOfRoot = (root) => state.cases.find((c) => c.applicationGramId === root) ?? null;
const caseText = (c) => (c ? t('kroniek.case', { law: c.lawName, id: c.id.slice(-5) }) : t('kroniek.case.unknown'));
const filteredCase = computed(() => (props.root ? caseText(caseOfRoot(props.root)) : ''));
const { label, linkable, openProvision } = useProvisionLinks(corpus, () => state.referenceDate);

/** Een moment uit een gram, met datum en tijd zoals de cel het schreef. */
function moment(iso) {
  if (!iso) return '';
  return `${formatDate(iso.slice(0, 10))} ${iso.slice(11, 16)}`;
}

/** Wat de wet zegt dat er ontstaat, in haar eigen begrippen (de afgeleide vorm). */
function lawSays(shape, gram) {
  const parts = [];
  const kind = shape?.subtype ?? gram.subtype;
  if (gram.type === 'submission' && kind) parts.push(t('kroniek.law.submission', { kind: kind.toUpperCase() }));
  const character = shape?.legal_character ?? gram.legal_character;
  if (character) parts.push(t('kroniek.law.character', { character }));
  const decisionType = shape?.decision_type ?? gram.decision_type;
  if (decisionType) parts.push(t('kroniek.law.decision_type', { type: decisionType }));
  const stage = shape?.stage ?? gram.stage;
  if (stage) parts.push(t('kroniek.law.stage', { stage }));
  return parts.join(' · ');
}

function describe(entry) {
  const { gram } = entry;
  const shape = demo.gramShape(gram);
  const c = caseOfRoot(entry.root);
  return {
    ...entry,
    shape,
    case: c,
    caseText: caseText(c),
    lawSays: lawSays(shape, gram),
    references: entry.references.map((r) => {
      const declared = shape?.refers_to?.[r.role];
      return {
        ...r,
        law: declared?.to ? t('kroniek.refers.to', { provision: label(declared.to) }) : declared?.stage ? t('kroniek.refers.stage', { stage: declared.stage }) : '',
      };
    }),
    articles: fieldsByArticle(shape?.fields, gram.field_basis),
    inputs: inputRows(gram, corpus.value).map((row) => ({ ...row, provenance: provenanceText(row.provenance) })),
    stored: JSON.stringify(gram, null, 2),
  };
}

/** De naam van de gram op plaats `position` in dezelfde kroniek. */
function nameAt(chronicle, position) {
  const target = chronicle.entries.find((e) => e.position === position);
  return target ? `${position}. ${humanize(target.gram.name)}` : `${position}`;
}

const open = reactive({});
const raw = reactive({});

/** Naar de gram waarnaar een andere verwijst, opengeklapt. */
async function show(id) {
  open[id] = true;
  await nextTick();
  document.getElementById(`gram-${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}
watch(() => props.focus, (id) => { if (id) show(id); }, { immediate: true });
</script>

<template>
  <nldd-container gap="16">
    <nldd-rich-text spacing="tight"><p><small>{{ t('kroniek.hint') }}</small></p></nldd-rich-text>
    <nldd-banner v-if="root" variant="neutral" :text="t('kroniek.filtered', { case: filteredCase })">
      <nldd-button slot="actions" size="sm" appearance="neutral-tinted" :text="t('kroniek.filtered.clear')" @click="emit('clear-root')"></nldd-button>
    </nldd-banner>
    <nldd-container v-if="!chronicles.length" padding-inline="12">
      <nldd-text size="sm" color="secondary">{{ t('kroniek.no_cell') }}</nldd-text>
    </nldd-container>

    <nldd-container v-for="chronicle in chronicles" :key="chronicle.cell.id" gap="12">
      <nldd-title size="4">
        <h2>{{ t('kroniek.of_cell', { cell: chronicle.cell.id }) }}</h2>
        <span slot="supporting-text">{{ t.plural(chronicle.total, 'kroniek.count', { actor: chronicle.cell.recordingActor ?? chronicle.cell.id }) }}</span>
      </nldd-title>
      <nldd-container v-if="!chronicle.entries.length" padding-inline="12">
        <nldd-text size="sm" color="secondary">{{ t('kroniek.empty') }}</nldd-text>
      </nldd-container>

      <nldd-container v-for="e in chronicle.entries" :id="`gram-${e.gram.id}`" :key="e.gram.id" gap="4">
        <nldd-list appearance="box-tinted" :accessible-label="humanize(e.gram.name)">
          <nldd-list-item size="md" button :expanded="!!open[e.gram.id]" @click="open[e.gram.id] = !open[e.gram.id]">
            <nldd-text-cell :text="`${e.position}. ${humanize(e.gram.name)}`" :supporting-text="e.caseText">
              <nldd-tag slot="overline" size="sm" color="neutral" :text="e.gram.type"></nldd-tag>
              <nldd-tag v-if="e.gram.stage" slot="overline" size="sm" color="accent" :text="e.gram.stage"></nldd-tag>
            </nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="moment(e.gram.recorded_at)" :supporting-text="t('kroniek.recorded_at')"></nldd-text-cell>
            <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
          </nldd-list-item>

          <template v-if="open[e.gram.id]">
            <!-- Wat de wet zegt: het artikel dat de gram vestigt, in de
                 begrippen van de wet zelf. -->
            <!-- Elke bepaling is een link naar haar artikel in Regelwerken,
                 behalve een wet die niet in het demo-corpus staat. -->
            <nldd-list-item size="sm" :button="linkable(e.gram.establishes) || undefined" @click="openProvision(e.gram.establishes)">
              <nldd-text-cell size="sm" :overline="t('kroniek.establishes')" :text="label(e.gram.establishes)" :supporting-text="e.lawSays"></nldd-text-cell>
              <nldd-icon-cell v-if="linkable(e.gram.establishes)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
            <!-- De rechtsgrond van de gram: het artikel dat hem vestigt. -->
            <nldd-list-item v-for="basis in e.gram.legal_basis ?? []" :key="`basis-${basis}`" size="sm" :button="linkable(basis) || undefined" @click="openProvision(basis)">
              <nldd-text-cell size="sm" :overline="t('kroniek.legal_basis')" :text="label(basis)"></nldd-text-cell>
              <nldd-icon-cell v-if="linkable(basis)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
            <!-- Per artikel dat velden vraagt: elk veld met zijn eigen
                 grondslag, zoals de gram die vastlegt. -->
            <template v-for="a in e.articles" :key="a.article">
              <nldd-list-item size="sm" :button="linkable(a.article) || undefined" @click="openProvision(a.article)">
                <nldd-text-cell size="sm" :overline="t('kroniek.fields_by')" :text="label(a.article)"></nldd-text-cell>
                <nldd-icon-cell v-if="linkable(a.article)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
              </nldd-list-item>
              <template v-for="f in a.fields" :key="`${a.article}-${f.name}`">
                <nldd-list-item v-if="!f.basis.length" size="sm">
                  <nldd-spacer-cell size="16"></nldd-spacer-cell>
                  <nldd-text-cell size="sm" :text="humanize(f.name)" :supporting-text="t('kroniek.field.no_basis')"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item v-for="basis in f.basis" :key="`${a.article}-${f.name}-${basis}`" size="sm" :button="linkable(basis) || undefined" @click="openProvision(basis)">
                  <nldd-spacer-cell size="16"></nldd-spacer-cell>
                  <nldd-text-cell size="sm" :text="humanize(f.name)" :supporting-text="label(basis)"></nldd-text-cell>
                  <nldd-icon-cell v-if="linkable(basis)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
                </nldd-list-item>
              </template>
            </template>

            <!-- De twee tijden van een gram: wanneer het feit rechtens telt,
                 en wanneer de cel het vastlegde. -->
            <nldd-list-item size="sm">
              <nldd-text-cell size="sm" :overline="t('kroniek.effective_at')" :text="e.gram.effective_at_legal_basis?.length ? t('kroniek.effective_at.basis') : t('kroniek.effective_at.no_basis')" :supporting-text="e.shape?.dated_by ? t('kroniek.dated_by', { parameter: humanize(e.shape.dated_by) }) : undefined"></nldd-text-cell>
              <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="moment(e.gram.effective_at)"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="basis in e.gram.effective_at_legal_basis ?? []" :key="`moment-${basis}`" size="sm" :button="linkable(basis) || undefined" @click="openProvision(basis)">
              <nldd-spacer-cell size="16"></nldd-spacer-cell>
              <nldd-text-cell size="sm" :text="label(basis)"></nldd-text-cell>
              <nldd-icon-cell v-if="linkable(basis)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
            <nldd-list-item size="sm">
              <nldd-text-cell size="sm" :overline="t('kroniek.recorded_at')" :text="e.gram.recording_actor"></nldd-text-cell>
              <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="moment(e.gram.recorded_at)"></nldd-text-cell>
            </nldd-list-item>

            <!-- De registratie: de stroom van de cel die de gebeurtenis een naam geeft. -->
            <nldd-list-item v-if="e.event" size="sm">
              <nldd-text-cell size="sm" :overline="t('kroniek.stream')" :text="e.event.stream ?? e.event.streamFile" :supporting-text="t('kroniek.stream.event', { event: e.event.name, establishes: e.event.establishes, file: e.event.streamFile }) + (e.event.stage ? ` · ${t('kroniek.law.stage', { stage: e.event.stage })}` : '')"></nldd-text-cell>
            </nldd-list-item>

            <nldd-list-item v-if="e.case" size="sm" button @click="goTo('zaaksysteem', { caseId: e.case.id })">
              <nldd-text-cell size="sm" :overline="t('kroniek.case.label')" :text="e.caseText"></nldd-text-cell>
              <nldd-icon-cell icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
            <nldd-list-item v-for="r in e.references" :key="r.role" size="sm" :button="r.position ? true : undefined" @click="r.position && show(r.id)">
              <nldd-text-cell size="sm" :overline="t('kroniek.refers', { role: humanize(r.role) })" :text="r.position ? nameAt(chronicle, r.position) : t('kroniek.refers.missing', { id: r.id })" :supporting-text="r.law || undefined"></nldd-text-cell>
              <nldd-icon-cell v-if="r.position" icon="chevron-up" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>

            <!-- Waarmee besloten is: elke invoer met haar herkomst, zoals de cel
                 haar vastlegt. -->
            <nldd-list-item v-for="input in e.inputs" :key="input.name" size="sm">
              <nldd-text-cell size="sm" :overline="t('kroniek.input')" :text="humanize(input.name)" :supporting-text="input.provenance || undefined"></nldd-text-cell>
              <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="input.text"></nldd-text-cell>
            </nldd-list-item>

            <nldd-list-item size="sm" button :expanded="!!raw[e.gram.id]" @click="raw[e.gram.id] = !raw[e.gram.id]">
              <nldd-icon-cell icon="code" size="16" color="secondary"></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell size="sm" :text="t('kroniek.raw')" :supporting-text="t('kroniek.raw.hint', { id: e.gram.id })"></nldd-text-cell>
              <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
          </template>
        </nldd-list>
        <nldd-code-viewer v-if="open[e.gram.id] && raw[e.gram.id]" language="json" wrap>{{ e.stored }}</nldd-code-viewer>
      </nldd-container>
    </nldd-container>
  </nldd-container>
</template>
