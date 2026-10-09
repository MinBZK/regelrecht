<script setup>
import { computed, reactive } from 'vue';
import { formatDate, humanize } from '../data/format.js';
import { cellsOfService } from '../data/chronolex.js';
import { serviceInfo } from '../data/loadCorpus.js';
import {
  derivationRows,
  filterRows,
  givesText,
  listText,
  policyArticles,
  readerOf,
  readingGrams,
  readingPeriods,
  readsPerCase,
  sourceText,
  valueRows,
} from '../data/lexostatusView.js';
import { onlyCase, storedChronicle } from '../data/storedChronicle.js';
import { useDemo } from '../store/demoStore.js';
import { useI18n } from '../i18n/index.js';
import { useProvisionLinks } from '../useProvisionLinks.js';

// De lexostatussen van de cellen van deze organisatie: wat een cel op een dag
// over een zaak weet, teruggelezen uit haar eigen kroniek. Per lexostatus in
// gewone woorden wat zij geeft, waar de cel het haalt, wie het gebruikt, per
// gegeven waar het vandaan komt en welke invoer het vult, en wat het nu is.
// Hoe de cel reduceert (filter, keuze, register) staat onder "Technisch".
// Welke lexostatus, welk veld of welk beleid het is, geeft de cel; deze
// component noemt er geen.

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
const { label, linkable, external, href, openProvision } = useProvisionLinks(corpus);

/** Een link naar een bepaling: binnen de demo via de router, anders een nieuw tabblad. */
function followProvision(event, provision) {
  if (external(provision)) return;
  event.preventDefault();
  openProvision(provision);
}

const caseOfRoot = (root) => state.cases.find((c) => c.applicationGramId === root) ?? null;
const caseText = (c) => (c ? t('kroniek.case', { law: c.lawName, id: c.id.slice(-5) }) : t('kroniek.case.unknown'));
const filteredCase = computed(() => (props.root ? caseText(caseOfRoot(props.root)) : ''));
const actor = computed(() => serviceInfo(corpus.value, props.service).name);
// Het regelwerk zoals het op de peildatum geldt: welke parameter een artikel
// vraagt, kan per versie verschillen.
const lawDoc = (id) => (corpus.value?.lawOn?.(id, state.referenceDate) ?? corpus.value?.lawById?.(id))?.doc ?? null;

/** Een regel van een filter zoals een mens hem leest. */
const filterText = (f) => (f.input ? t('lexo.filter.input', { key: f.key, input: f.input }) : t('lexo.filter.value', { key: f.key, value: f.value }));
/** De regel van een afleiding zoals een mens hem leest. */
const ruleText = (d) => (d.rule ? t(`lexo.rule.${d.rule}`, { of: d.of }) : t('lexo.rule.unknown'));
/** Wie een lexostatus leest: de gebeurtenis, met haar fase en stroom. */
const readerText = (r) => [r.stage ? t('kroniek.law.stage', { stage: r.stage }) : '', t('lexo.read_by.stream', { stream: r.stream })].filter(Boolean).join(' · ');
/** Een gram waaruit een lezing kwam, met zijn plaats in de kroniek en zijn datum. */
const gramText = (g) => (g.position ? t('lexo.from_gram.text', { position: g.position, name: humanize(g.name), date: formatDate(g.date) }) : t('kroniek.refers.missing', { id: g.id }));

/**
 * Een grondslag zoals de tabel hem toont: bij een beleid dat de lexostatus
 * zelf is alleen het artikel, want de naam van het beleid staat al bij de bron.
 */
const basisText = (l, provision) =>
  l.kind === 'policy' && provision.startsWith(`${l.name}#`) ? t('lexo.basis.article', { article: provision.slice(l.name.length + 1) }) : label(provision);

/** Welke lexostatussen "Technisch" open hebben. */
const technical = reactive(new Set());
const toggleTechnical = (key) => (technical.has(key) ? technical.delete(key) : technical.add(key));

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
      lexostatuses: lexostatuses.map((l) => {
        const readers = l.read_by.map((r) => readerOf(r, demo.eventShapeOf(cell.id, r.event)));
        return {
          ...l,
          key: `${cell.id}:${l.name}`,
          title: l.kind === 'policy' ? label(l.name) : humanize(l.name),
          gives: givesText(l, readers),
          source: sourceText(l, actor.value),
          usedBy: listText(readers.map((r) => r.label)),
          values: valueRows(l, readers, lawDoc),
          filter: l.kind === 'configuration' ? filterRows(l.reduction.filter) : [],
          derivations: l.kind === 'configuration' ? derivationRows(l.reduction.derivations) : [],
          articles: l.kind === 'policy' ? policyArticles(l, lawDoc(l.name)) : [],
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
        };
      }),
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
        <h2>{{ t('lexo.of_cell', { actor }) }}</h2>
        <span slot="supporting-text">{{ t.plural(c.lexostatuses.length, 'lexo.count', { actor }) }}</span>
      </nldd-title>
      <nldd-banner v-if="c.error" variant="warning" :text="t('chronicle.read_failed')" :supporting-text="c.error"></nldd-banner>
      <nldd-container v-else-if="!c.lexostatuses.length" padding-inline="12">
        <nldd-text size="sm" color="secondary">{{ t('lexo.none') }}</nldd-text>
      </nldd-container>

      <nldd-container v-for="l in c.lexostatuses" :key="l.key" gap="8">
        <!-- 1. Wat zij is en wat zij geeft. -->
        <nldd-title size="5" heading-level="3" :text="l.title" :supporting-text="l.gives"></nldd-title>

        <!-- 2. Waar de cel het haalt, en 3. wie het gebruikt. -->
        <nldd-list appearance="box-tinted" :accessible-label="l.title">
          <nldd-list-item size="sm" :button="(l.kind === 'policy' && linkable(l.name)) || undefined" @click="l.kind === 'policy' && openProvision(l.name)">
            <nldd-text-cell size="sm" :overline="t('lexo.section.source')" :text="l.source" :supporting-text="l.kind === 'policy' ? label(l.name) : undefined"></nldd-text-cell>
            <nldd-icon-cell v-if="l.kind === 'policy' && linkable(l.name)" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
          </nldd-list-item>
          <nldd-list-item size="sm">
            <nldd-text-cell size="sm" :overline="t('lexo.section.used_by')" :text="l.usedBy || t('lexo.used_by.none')"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>

        <!-- 4. Per gegeven: waar het vandaan komt, waarvoor het dient, grondslag. -->
        <nldd-table
          v-if="l.values.length"
          columns="minmax(140px,1fr) minmax(170px,1fr) minmax(180px,1.3fr) minmax(220px,1.6fr) minmax(200px,1.3fr)"
          :accessible-label="t('lexo.table', { lexostatus: l.title })"
        >
          <nldd-table-row slot="header">
            <nldd-text-cell size="sm" :text="t('lexo.col.value')"></nldd-text-cell>
            <nldd-text-cell size="sm" :text="t('lexo.col.name')"></nldd-text-cell>
            <nldd-text-cell size="sm" :text="t('lexo.col.origin')"></nldd-text-cell>
            <nldd-text-cell size="sm" :text="t('lexo.col.used_as')"></nldd-text-cell>
            <nldd-text-cell size="sm" :text="t('lexo.col.basis')"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="v in l.values" :key="v.name">
            <nldd-text-cell size="sm" :text="v.label"></nldd-text-cell>
            <nldd-text-cell size="sm" color="secondary" :text="v.name"></nldd-text-cell>
            <nldd-text-cell size="sm" :text="v.origin"></nldd-text-cell>
            <nldd-cell width="full">
              <nldd-container gap="4">
                <nldd-link
                  v-for="u in v.uses"
                  :key="u.provision"
                  size="sm"
                  :href="href(u.provision)"
                  :target="external(u.provision) ? '_blank' : undefined"
                  :text="t('lexo.used_as', { article: label(u.provision), readers: listText(u.readers) })"
                  @click="followProvision($event, u.provision)"
                ></nldd-link>
              </nldd-container>
            </nldd-cell>
            <nldd-cell width="full">
              <nldd-container gap="4">
                <template v-for="b in v.basis" :key="b">
                  <nldd-link
                    v-if="linkable(b)"
                    size="sm"
                    :href="href(b)"
                    :target="external(b) ? '_blank' : undefined"
                    :end-icon="external(b) ? 'external-link' : undefined"
                    :text="basisText(l, b)"
                    @click="followProvision($event, b)"
                  ></nldd-link>
                  <nldd-text v-else size="sm">{{ basisText(l, b) }}</nldd-text>
                </template>
              </nldd-container>
            </nldd-cell>
          </nldd-table-row>
        </nldd-table>

        <!-- 5. Wat zij nu geeft, per zaak (en per berekeningsjaar), met de grammen waaruit het komt. -->
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
            <nldd-list-item v-for="row in r.rows" :key="`v-${row.name}`" size="sm">
              <nldd-text-cell size="sm" :text="humanize(row.name)" :supporting-text="row.name"></nldd-text-cell>
              <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="row.text"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-if="!r.rows.length" size="sm">
              <nldd-text-cell size="sm" :text="t('lexo.outcome.empty')"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="g in r.grams" :key="`g-${g.id}`" size="sm" :button="g.position ? true : undefined" @click="g.position && emit('show-gram', { id: g.id, root: r.root })">
              <nldd-text-cell size="sm" :overline="t('lexo.from_gram')" :text="gramText(g)"></nldd-text-cell>
              <nldd-icon-cell v-if="g.position" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
            <nldd-list-item v-if="!r.grams.length" size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.from_gram')" :text="t('lexo.from_gram.none')"></nldd-text-cell>
            </nldd-list-item>
          </template>
        </nldd-list>

        <!-- 6. Technisch: hoe de cel reduceert, zoals in haar configuratie. -->
        <nldd-container>
          <nldd-button
            size="sm"
            appearance="neutral-transparent"
            :end-icon="technical.has(l.key) ? 'chevron-up' : 'chevron-down'"
            :text="t(technical.has(l.key) ? 'lexo.technical.hide' : 'lexo.technical.show')"
            @click="toggleTechnical(l.key)"
          ></nldd-button>
        </nldd-container>
        <nldd-list v-if="technical.has(l.key)" appearance="box-tinted" :accessible-label="t('lexo.technical.label', { lexostatus: l.title })">
          <nldd-list-item size="sm">
            <nldd-text-cell size="sm" :overline="t('lexo.technical.name')" :text="l.name" :supporting-text="t(`lexo.kind.${l.kind}`)"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item v-for="r in l.read_by" :key="`by-${r.event}`" size="sm">
            <nldd-text-cell size="sm" :overline="t('lexo.read_by')" :text="r.event" :supporting-text="readerText(r)"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="sm">
            <nldd-text-cell size="sm" :overline="t('lexo.inputs')" :text="l.inputs.length ? l.inputs.join(', ') : t('lexo.inputs.none')"></nldd-text-cell>
          </nldd-list-item>

          <!-- In de configuratie van de cel. -->
          <template v-if="l.kind === 'configuration'">
            <nldd-list-item size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.filter')" :text="t('lexo.filter.text', { chronicle: l.reduction.chronicle })" :supporting-text="l.filter.map(filterText).join(' · ')"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.pick')" :text="t(`lexo.pick.${l.reduction.pick}`)" :supporting-text="t(`lexo.pick.${l.reduction.pick}.supporting`)"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="d in l.derivations" :key="`d-${d.name}`" size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.derivation')" :text="d.name" :supporting-text="ruleText(d)"></nldd-text-cell>
            </nldd-list-item>
          </template>

          <!-- Met een artikel in het beleid van de houder dat de kroniek als register leest. -->
          <template v-else-if="l.kind === 'policy'">
            <nldd-list-item size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.register')" :text="`${l.name}#${l.register}`" :supporting-text="t('lexo.register.supporting', { chronicle: l.chronicle, input: l.register_input })"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="a in l.articles" :key="`a-${a.number}`" size="sm">
              <nldd-text-cell size="sm" :overline="t('lexo.article')" :text="a.provision" :supporting-text="a.outputs.map((o) => o.name).join(', ')"></nldd-text-cell>
            </nldd-list-item>
          </template>
        </nldd-list>
      </nldd-container>
    </nldd-container>
  </nldd-container>
</template>
