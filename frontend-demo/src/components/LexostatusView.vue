<script setup>
import { computed } from 'vue';
import { formatDate, humanize } from '../data/format.js';
import { cellsOfService } from '../data/chronolex.js';
import { derivationRows, filterRows, policyArticles, readingGrams, readingPeriods, readsPerCase } from '../data/lexostatusView.js';
import { onlyCase, storedChronicle } from '../data/storedChronicle.js';
import { useDemo } from '../store/demoStore.js';
import { useI18n } from '../i18n/index.js';
import { useProvisionLinks } from '../useProvisionLinks.js';

// De lexostatussen van de cellen van deze organisatie: hoe een cel haar eigen
// kroniek terugleest. Per lexostatus wie haar leest, hoe zij reduceert (in de
// configuratie van de cel, of met een artikel in het beleid van de houder dat
// de kroniek als register leest) en wat dat nu oplevert, per zaak, met de
// grammen waaruit het komt. Welke lexostatus, welk veld of welk beleid het is,
// geeft de cel; deze component noemt er geen.

const props = defineProps({
  /** De organisatie waarvan de cellen getoond worden. */
  service: { type: String, default: null },
  /** Alleen de zaak die met deze gram begon; leeg = elke zaak. */
  root: { type: String, default: null },
});
const emit = defineEmits(['clear-root', 'show-gram']);

const { t } = useI18n();
const demo = useDemo();
const { corpus, state, dataVersion } = demo;
const { label, linkable, openProvision } = useProvisionLinks(corpus);

const caseOfRoot = (root) => state.cases.find((c) => c.applicationGramId === root) ?? null;
const caseText = (c) => (c ? t('kroniek.case', { law: c.lawName, id: c.id.slice(-5) }) : t('kroniek.case.unknown'));
const filteredCase = computed(() => (props.root ? caseText(caseOfRoot(props.root)) : ''));

/** Een regel van een filter zoals een mens hem leest. */
const filterText = (f) => (f.input ? t('lexo.filter.input', { key: f.key, input: f.input }) : t('lexo.filter.value', { key: f.key, value: f.value }));
/** De regel van een afleiding zoals een mens hem leest. */
const ruleText = (d) => (d.rule ? t(`lexo.rule.${d.rule}`, { of: d.of }) : t('lexo.rule.unknown'));
/** Wie een lexostatus leest: de gebeurtenis, met haar fase en stroom. */
const readerText = (r) => [r.stage ? t('kroniek.law.stage', { stage: r.stage }) : '', t('lexo.read_by.stream', { stream: r.stream })].filter(Boolean).join(' · ');

const cells = computed(() => {
  void dataVersion.value;
  void state.referenceDate;
  return cellsOfService(corpus.value, props.service).map((cell) => {
    const entries = storedChronicle(cell, state.grams);
    // De zaken in deze kroniek: elke gram waar een zaak mee begon.
    const roots = [...new Set(onlyCase(entries, props.root).map((e) => e.root))];
    const { lexostatuses, error } = demo.lexostatusesOf(cell.id);
    return {
      cell,
      error,
      lexostatuses: lexostatuses.map((l) => ({
        ...l,
        filter: l.kind === 'configuration' ? filterRows(l.reduction.filter) : [],
        derivations: l.kind === 'configuration' ? derivationRows(l.reduction.derivations) : [],
        articles: l.kind === 'policy' ? policyArticles(l, corpus.value?.lawById?.(l.name)?.doc) : [],
        perCase: readsPerCase(l),
        // Per zaak; leest de lexostatus per periode (een berekeningsjaar),
        // dan per periode van de zaak.
        readings: readsPerCase(l)
          ? roots.flatMap((root) => {
              const grams = onlyCase(entries, root).map((e) => e.gram);
              return readingPeriods(l, grams).map((period) => {
                const reading = demo.readLexostatusOf(cell.id, l, root, period);
                const text = caseText(caseOfRoot(root));
                return {
                  root,
                  period,
                  caseText: period != null ? t('lexo.reading.period', { case: text, period }) : text,
                  ...reading,
                  grams: readingGrams(reading.grams, entries),
                };
              });
            })
          : [],
      })),
    };
  });
});
</script>

<template>
  <nldd-container gap="16">
    <nldd-rich-text spacing="tight"><p><small>{{ t('lexo.hint') }}</small></p></nldd-rich-text>
    <nldd-banner v-if="root" variant="neutral" :text="t('kroniek.filtered', { case: filteredCase })">
      <nldd-button slot="actions" size="sm" appearance="neutral-tinted" :text="t('kroniek.filtered.clear')" @click="emit('clear-root')"></nldd-button>
    </nldd-banner>
    <nldd-container v-if="!cells.length" padding-inline="12">
      <nldd-text size="sm" color="secondary">{{ t('kroniek.no_cell') }}</nldd-text>
    </nldd-container>

    <nldd-container v-for="c in cells" :key="c.cell.id" gap="16">
      <nldd-title size="4">
        <h2>{{ t('lexo.of_cell', { cell: c.cell.id }) }}</h2>
        <span slot="supporting-text">{{ t.plural(c.lexostatuses.length, 'lexo.count', { actor: c.cell.recordingActor ?? c.cell.id }) }}</span>
      </nldd-title>
      <nldd-banner v-if="c.error" variant="warning" :text="t('chronicle.read_failed')" :supporting-text="c.error"></nldd-banner>
      <nldd-container v-else-if="!c.lexostatuses.length" padding-inline="12">
        <nldd-text size="sm" color="secondary">{{ t('lexo.none') }}</nldd-text>
      </nldd-container>

      <nldd-container v-for="l in c.lexostatuses" :key="l.name" gap="8">
        <nldd-title size="5" heading-level="3" :text="l.kind === 'policy' ? label(l.name) : humanize(l.name)" :supporting-text="t(`lexo.kind.${l.kind}`)"></nldd-title>
        <nldd-list appearance="box-tinted" :accessible-label="l.name">
          <!-- Wat zij is: wie haar leest en haar invoer. -->
          <nldd-list-item v-for="r in l.read_by" :key="`by-${r.event}`" size="sm">
            <nldd-text-cell size="sm" :overline="t('lexo.read_by')" :text="humanize(r.event)" :supporting-text="readerText(r)"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item v-if="!l.read_by.length" size="sm">
            <nldd-text-cell size="sm" :overline="t('lexo.read_by')" :text="t('lexo.read_by.none')"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="sm">
            <nldd-text-cell size="sm" :overline="t('lexo.inputs')" :text="l.inputs.length ? l.inputs.join(', ') : t('lexo.inputs.none')"></nldd-text-cell>
          </nldd-list-item>

          <!-- Hoe zij reduceert, in de configuratie van de cel. -->
          <template v-if="l.kind === 'configuration'">
            <nldd-list-item size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.filter')" :text="t('lexo.filter.text', { chronicle: l.reduction.chronicle })" :supporting-text="l.filter.map(filterText).join(' · ')"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.pick')" :text="t(`lexo.pick.${l.reduction.pick}`)" :supporting-text="t(`lexo.pick.${l.reduction.pick}.supporting`)"></nldd-text-cell>
            </nldd-list-item>
            <template v-for="d in l.derivations" :key="`d-${d.name}`">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" :overline="t('lexo.derivation')" :text="humanize(d.name)" :supporting-text="ruleText(d)"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item v-for="basis in d.legalBasis" :key="`d-${d.name}-${basis}`" size="sm" :button="linkable(basis) || undefined" @click="openProvision(basis)">
                <nldd-spacer-cell size="16"></nldd-spacer-cell>
                <nldd-text-cell size="sm" :overline="t('kroniek.legal_basis')" :text="label(basis)"></nldd-text-cell>
                <nldd-icon-cell v-if="linkable(basis)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
              </nldd-list-item>
            </template>
          </template>

          <!-- Hoe zij reduceert, met een artikel in het beleid van de houder. -->
          <template v-else-if="l.kind === 'policy'">
            <nldd-list-item size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.register')" :text="`${l.name}#${l.register}`" :supporting-text="t('lexo.register.supporting', { chronicle: l.chronicle, input: l.register_input })"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="a in l.articles" :key="`a-${a.number}`" size="sm" :button="linkable(a.provision) || undefined" @click="openProvision(a.provision)">
              <nldd-text-cell size="sm" :overline="t('lexo.article')" :text="label(a.provision)" :supporting-text="a.outputs.map((o) => humanize(o.name)).join(', ')"></nldd-text-cell>
              <nldd-icon-cell v-if="linkable(a.provision)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
          </template>
        </nldd-list>

        <!-- Wat zij nu oplevert, per zaak, met de grammen waaruit het komt. -->
        <nldd-text size="sm" weight="medium" color="secondary">{{ t('lexo.outcome', { date: formatDate(state.referenceDate) }) }}</nldd-text>
        <nldd-container v-if="!l.perCase" padding-inline="12">
          <nldd-text size="sm" color="secondary">{{ t('lexo.outcome.not_per_case') }}</nldd-text>
        </nldd-container>
        <nldd-container v-else-if="!l.readings.length" padding-inline="12">
          <nldd-text size="sm" color="secondary">{{ t('lexo.outcome.no_cases') }}</nldd-text>
        </nldd-container>
        <nldd-list v-for="r in l.readings" :key="`r-${r.root}-${r.period}`" appearance="box-tinted" :accessible-label="r.caseText">
          <nldd-list-item size="sm">
            <nldd-text-cell size="sm" :overline="t('kroniek.case.label')" :text="r.caseText"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item v-if="r.error" size="sm">
            <nldd-text-cell size="sm" :text="t('lexo.read_failed')" :supporting-text="r.error"></nldd-text-cell>
          </nldd-list-item>
          <template v-else>
            <nldd-list-item v-for="row in r.rows" :key="`v-${row.name}`" size="sm" :button="(row.article && linkable(row.article)) || undefined" @click="row.article && openProvision(row.article)">
              <nldd-text-cell size="sm" :text="humanize(row.name)" :supporting-text="row.article ? label(row.article) : undefined"></nldd-text-cell>
              <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="row.text"></nldd-text-cell>
              <nldd-icon-cell v-if="row.article && linkable(row.article)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
            <nldd-list-item v-if="!r.rows.length" size="sm">
              <nldd-text-cell size="sm" :text="t('lexo.outcome.empty')"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="g in r.grams" :key="`g-${g.id}`" size="sm" :button="g.position ? true : undefined" @click="g.position && emit('show-gram', { id: g.id, root: r.root })">
              <nldd-text-cell size="sm" :overline="t('lexo.from_gram')" :text="g.position ? `${g.position}. ${humanize(g.name)}` : t('kroniek.refers.missing', { id: g.id })"></nldd-text-cell>
              <nldd-icon-cell v-if="g.position" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
            <nldd-list-item v-if="!r.grams.length" size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.from_gram')" :text="t('lexo.from_gram.none')"></nldd-text-cell>
            </nldd-list-item>
          </template>
        </nldd-list>
      </nldd-container>
    </nldd-container>
  </nldd-container>
</template>
