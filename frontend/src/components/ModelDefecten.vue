<script setup>
/**
 * ModelDefecten: de defecten die de engine in het model vindt, als lijst.
 *
 * Tot nu toe waren alleen de aantallen zichtbaar, in de legenda onder de graaf.
 * Je kon dus zien dát er drie kapotte bindingen zijn en nooit welke. Dit blok
 * toont de rijen, elk met een link naar het artikel waar het defect zit, want
 * daar wordt het gerepareerd.
 *
 * Losstaand van de ankerfouten in `NotenRapport`. Die gaan over notities die
 * hun tekst niet meer vinden; deze gaan over het model zelf. Ze delen het woord
 * "bevinding" in het Nederlands, en juist daarom heten ze hier anders.
 */
import { computed } from 'vue';

const props = defineProps({
  /** `findings` uit het rapport van `corpusMetrics()`. */
  defecten: { type: Array, default: () => [] },
  naamVoor: { type: Function, default: (lawId) => lawId },
  /** (lawId, artikel|null) -> href, of undefined voor niet-klikbare rijen. */
  hrefVoor: { type: Function, default: () => undefined },
});

/**
 * Nederlandse kop per klasse, plus wat er aan de hand is.
 *
 * De klassenamen komen uit de engine en blijven Engels, want dat is de sleutel.
 * Op het scherm hoort te staan wat het betekent, niet hoe het veld heet.
 */
const KLASSE = {
  dangling: {
    kop: 'Verwijzing naar een output die niet bestaat',
    uitleg: 'De doelregeling levert deze waarde niet, dus de uitvoering loopt hier vast.',
  },
  'impl-dangling': {
    kop: 'Invulling wijst naar een open term die er niet is',
    uitleg: 'De delegatie resolvet nooit: het aangewezen artikel declareert deze term niet.',
  },
  'impl-no-date': {
    kop: 'Invullende regeling zonder ingangsdatum',
    uitleg: 'Zonder datum geldt zij op elke rekendatum en overschrijft zij stil de juiste versie.',
  },
  'plain-param': {
    kop: 'Parameter noemt een andere regeling maar haalt er niets op',
    uitleg: 'De beschrijving verwijst naar een regeling; er is geen binding die dat waarmaakt.',
  },
  misplaced: {
    kop: 'Verwijzing op de verkeerde plek',
    uitleg: 'Een source onder parameters wordt door de engine genegeerd; verplaats hem naar input.',
  },
  'open-term-unfilled': {
    kop: 'Open term die niemand invult',
    uitleg: 'De delegatie is gedeclareerd maar geen enkele geladen regeling vult haar in.',
  },
};

const groepen = computed(() => {
  const per = new Map();
  for (const d of props.defecten) {
    if (!per.has(d.class)) per.set(d.class, []);
    per.get(d.class).push(d);
  }
  // Zwaarste eerst: een kapotte binding vraagt eerder aandacht dan een open
  // term die nog niemand heeft ingevuld.
  const volgorde = ['dangling', 'impl-dangling', 'misplaced', 'impl-no-date', 'plain-param', 'open-term-unfilled'];
  return [...per.entries()]
    .sort(([a], [b]) => volgorde.indexOf(a) - volgorde.indexOf(b))
    .map(([klasse, rijen]) => ({
      klasse,
      kop: KLASSE[klasse]?.kop ?? klasse,
      uitleg: KLASSE[klasse]?.uitleg ?? '',
      rijen,
    }));
});

function rijTitel(d) {
  const naam = props.naamVoor(d.law_id);
  return d.article ? `${naam} · artikel ${d.article}` : naam;
}
</script>

<template>
  <nldd-simple-section width="800px">
    <nldd-title size="6"><h3>Defecten in het model</h3></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>

    <nldd-inline-dialog
      v-if="defecten.length === 0"
      data-test="geen-defecten"
      text="Geen defecten gevonden"
      supporting-text="Elke verwijzing tussen regelingen komt uit bij iets dat bestaat, en elke open term wordt ingevuld."
    ></nldd-inline-dialog>

    <template v-for="groep in groepen" :key="groep.klasse">
      <nldd-banner
        variant="warning"
        :text="groep.rijen.length === 1 ? `1 keer: ${groep.kop.toLowerCase()}` : `${groep.rijen.length} keer: ${groep.kop.toLowerCase()}`"
        :supporting-text="groep.uitleg"
      ></nldd-banner>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="simple" arrow-navigation>
        <nldd-list-item
          v-for="(d, i) in groep.rijen"
          :key="`${groep.klasse}-${d.law_id}-${d.article}-${i}`"
          size="md"
          button
          data-test="defect"
          :href="hrefVoor(d.law_id, d.article)"
        >
          <nldd-text-cell :text="rijTitel(d)" :supporting-text="d.detail"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="16"></nldd-spacer>
    </template>
    <nldd-spacer size="8"></nldd-spacer>
  </nldd-simple-section>
</template>
