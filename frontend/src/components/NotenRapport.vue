<script setup>
/**
 * NotenRapport: de presentatie van de notitielaag op de Analyse-pagina.
 *
 * Bevat geen fetch en geen router: het rapport komt als prop binnen en de
 * link per rij wordt door de aanroeper gemaakt. Daardoor is dit component
 * zonder backend te previewen (`analyse-preview.html`) en zonder mocks te
 * testen, terwijl `AnalyseView.vue` het echte werk aanlevert.
 */
import { computed, ref } from 'vue';

// De W3C-motivations uit het annotatieschema (RFC-005) blijven in het bestand
// Engels, want dat is de standaard. Op het scherm hoort Nederlands. Een soort
// die hier niet in staat valt terug op zijn eigen naam; dat is zichtbaar en
// oplosbaar, en beter dan een lege cel.
const SOORT_LABEL = {
  questioning: 'vraag',
  commenting: 'toelichting',
  linking: 'koppeling',
  assessing: 'beoordeling',
  describing: 'beschrijving',
  editing: 'wijzigingsvoorstel',
  highlighting: 'markering',
  classifying: 'classificatie',
  identifying: 'identificatie',
  tagging: 'tag',
  bookmarking: 'bladwijzer',
  moderating: 'moderatie',
  replying: 'reactie',
};

const props = defineProps({
  /** Uitvoer van `aggregeer()` uit `lib/notenanalyse.js`. */
  rapport: { type: Object, required: true },
  /** lawId -> leesbare naam. */
  naamVoor: { type: Function, default: (lawId) => lawId },
  /** (lawId, artikel|null) -> href, of null voor niet-klikbare rijen. */
  hrefVoor: { type: Function, default: () => undefined },
});

const kpis = computed(() => {
  const r = props.rapport;
  // "over 3 wetten" leest als volledige dekking; "over 3 van de 24" niet.
  // Als het corpustotaal onbekend is zeggen we het kleinere ding.
  const dekking =
    r.wettenInCorpus != null
      ? `over ${r.wettenMetNoten} van de ${r.wettenInCorpus} wetten`
      : `over ${r.wettenMetNoten} ${r.wettenMetNoten === 1 ? 'wet' : 'wetten'}`;
  return [
    { label: 'Notities in dit traject', waarde: r.totaal, onder: dekking },
    { label: 'Nog open', waarde: r.open, onder: `${r.totaal - r.open} afgehandeld` },
  ];
});

function soortLabel(sleutel) {
  return SOORT_LABEL[sleutel] ?? sleutel;
}

/**
 * De actieve filter over de notitielijst: een soort, een tag, of niets.
 *
 * Klikken op een groepsrij zet de filter, opnieuw klikken haalt hem weg. Zo is
 * elk getal in de twee kaarten een ingang naar de notities erachter, in plaats
 * van een dood aantal.
 */
const filter = ref(null);

function zetFilter(soort, waarde) {
  const zelfde = filter.value?.soort === soort && filter.value?.waarde === waarde;
  filter.value = zelfde ? null : { soort, waarde };
}

const zichtbareNoten = computed(() => {
  const alle = props.rapport.noten ?? [];
  const f = filter.value;
  if (!f) return alle;
  if (f.soort === 'motivation') return alle.filter((n) => n.soort === f.waarde);
  return alle.filter((n) => n.tags.includes(f.waarde));
});

const filterOmschrijving = computed(() => {
  const f = filter.value;
  if (!f) return null;
  if (f.soort === 'motivation') return `soort: ${soortLabel(f.waarde)}`;
  const tag = (props.rapport.naarTag ?? []).find((t) => t.id === f.waarde);
  return `tag: ${tag?.label ?? f.waarde}`;
});

/** Waar een notitie bij hoort, in één regel. */
function notitieTitel(n) {
  const naam = props.naamVoor(n.lawId);
  return n.artikel ? `${naam} · artikel ${n.artikel}` : naam;
}

/** Wat de notitie zegt, of waar zij naar wijst. */
function notitieInhoud(n) {
  if (n.inhoud?.soort === 'tekst') return n.inhoud.waarde;
  if (n.inhoud?.soort === 'verwijzing') return `wijst naar ${n.inhoud.waarde}`;
  return null;
}

function rijTitel(item) {
  const naam = props.naamVoor(item.lawId);
  return item.artikel ? `${naam} · artikel ${item.artikel}` : naam;
}
</script>

<template>
  <nldd-simple-section v-if="rapport.totaal === 0" width="800px">
    <nldd-inline-dialog
      text="Nog geen notities in dit traject"
      supporting-text="Zodra er notities bij artikelen staan, verschijnen ze hier gegroepeerd."
    ></nldd-inline-dialog>
  </nldd-simple-section>

  <template v-else>
    <!-- Totalen. Dezelfde half/half-ritmiek als Corpusinwinning, zodat elke
         kaart op de pagina één breedte deelt. -->
    <nldd-one-half-one-half-section>
      <nldd-card v-for="(kpi, i) in kpis" :key="kpi.label" :slot="i === 0 ? 'left' : 'right'">
        <nldd-container padding="16">
          <nldd-title size="2">
            <span slot="overline">{{ kpi.label }}</span>
            {{ kpi.waarde }}
            <span slot="subtitle">{{ kpi.onder }}</span>
          </nldd-title>
        </nldd-container>
      </nldd-card>
    </nldd-one-half-one-half-section>

    <!-- Soort en ambiguïteit naast elkaar: twee assen over dezelfde notities,
         geen opeenvolgende stappen. -->
    <nldd-one-half-one-half-section>
      <nldd-title slot="header" size="6"><h3>Waar de notities over gaan</h3></nldd-title>

      <nldd-card slot="left">
        <nldd-container padding="16">
          <nldd-title size="3">
            Naar soort
            <span slot="subtitle">wat de notitie doet</span>
          </nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-list variant="simple">
            <nldd-list-item
              v-for="rij in rapport.naarSoort"
              :key="rij.key"
              size="md"
              button
              data-test="soort"
              :selected="filter?.soort === 'motivation' && filter?.waarde === rij.key || undefined"
              @click="zetFilter('motivation', rij.key)"
            >
              <nldd-text-cell :text="soortLabel(rij.key)"></nldd-text-cell>
              <nldd-text-cell :text="String(rij.n)" horizontal-alignment="right"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-card>

      <nldd-card slot="right">
        <nldd-container padding="16">
          <nldd-title size="3">
            Naar ambiguïteit
            <span slot="subtitle">waarom een norm nog niet uitvoerbaar is</span>
          </nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-inline-dialog
            v-if="rapport.naarTag.length === 0"
            text="Geen tags"
            supporting-text="Een notitie bij een open norm kan een tag dragen die zegt waarom die norm nog niet uitvoerbaar is."
          ></nldd-inline-dialog>
          <nldd-list v-else variant="simple">
            <nldd-list-item
              v-for="tag in rapport.naarTag"
              :key="tag.id"
              size="md"
              button
              data-test="tag"
              :selected="filter?.soort === 'tag' && filter?.waarde === tag.id || undefined"
              @click="zetFilter('tag', tag.id)"
            >
              <nldd-icon-cell v-if="!tag.inVocabulaire" slot="start" size="20">
                <nldd-icon name="warning"></nldd-icon>
              </nldd-icon-cell>
              <nldd-text-cell
                :text="tag.label"
                :supporting-text="tag.inVocabulaire ? undefined : 'niet in het vocabulaire'"
              ></nldd-text-cell>
              <nldd-text-cell :text="String(tag.n)" horizontal-alignment="right"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-card>
    </nldd-one-half-one-half-section>

    <!-- Ankerfouten: het enige blok dat om actie vraagt. Een notitie die
         losgeraakt is van haar tekst wijst meestal op een wetswijziging die
         nog niet is nagelopen. -->
    <nldd-simple-section width="800px">
      <nldd-title size="6"><h3>Ankerfouten</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>

      <nldd-inline-dialog
        v-if="rapport.ankerfouten.items.length === 0 && rapport.ongemeten.length === 0"
        text="Elke notitie vindt haar tekst"
        supporting-text="De aangehaalde passage is voor elke notitie eenduidig terug te vinden in de wet."
      ></nldd-inline-dialog>

      <template v-else>
        <nldd-banner
          v-if="rapport.ankerfouten.items.length"
          variant="warning"
          :text="`${rapport.ankerfouten.orphaned} losgeraakt, ${rapport.ankerfouten.ambiguous} niet eenduidig`"
          supporting-text="De aangehaalde tekst is niet meer of niet eenduidig in de wet te vinden. Meestal is de wettekst gewijzigd nadat de notitie geschreven werd."
        ></nldd-banner>
        <nldd-spacer v-if="rapport.ankerfouten.items.length" size="12"></nldd-spacer>

        <nldd-list v-if="rapport.ankerfouten.items.length" variant="simple" arrow-navigation>
          <nldd-list-item
            v-for="(item, i) in rapport.ankerfouten.items"
            :key="`${item.lawId}-${item.artikel}-${i}`"
            size="md"
            button
            data-test="ankerfout"
            :href="hrefVoor(item.lawId, item.artikel)"
          >
            <nldd-text-cell :text="rijTitel(item)" :supporting-text="item.exact"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell
              :text="item.status === 'orphaned' ? 'losgeraakt' : 'niet eenduidig'"
              horizontal-alignment="right"
            ></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>

        <!-- "Niet gemeten" is nadrukkelijk niet hetzelfde als "in orde":
             zonder dit onderscheid leest een traject waarvan de engine geen
             wet kon laden als een gezond traject. -->
        <template v-if="rapport.ongemeten.length">
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-inline-dialog
            data-test="ongemeten"
            text="Niet gemeten"
            :supporting-text="`De engine kon deze wetten niet laden, dus hun ankers zijn niet gecontroleerd: ${rapport.ongemeten.join(', ')}`"
          ></nldd-inline-dialog>
        </template>
      </template>
    </nldd-simple-section>

    <!-- De notities zelf. Dit is waar een getal uit de kaarten hierboven op
         uitkomt: welke bepaling, welke passage, wie het schreef. -->
    <nldd-simple-section width="800px">
      <nldd-title size="6"><h3>De notities</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>

      <template v-if="filterOmschrijving">
        <nldd-inline-dialog
          data-test="filter"
          :text="`Gefilterd op ${filterOmschrijving}`"
          :supporting-text="`${zichtbareNoten.length} van ${(rapport.noten ?? []).length} notities. Klik de rij hierboven opnieuw aan om alles te tonen.`"
        ></nldd-inline-dialog>
        <nldd-spacer size="12"></nldd-spacer>
      </template>

      <nldd-inline-dialog
        v-if="zichtbareNoten.length === 0"
        text="Geen notities in deze selectie"
      ></nldd-inline-dialog>

      <nldd-list v-else variant="simple" arrow-navigation>
        <nldd-list-item
          v-for="(n, i) in zichtbareNoten"
          :key="`${n.lawId}-${n.artikel}-${i}`"
          size="md"
          button
          data-test="notitie"
          :href="hrefVoor(n.lawId, n.artikel)"
        >
          <nldd-text-cell
            :text="notitieTitel(n)"
            :supporting-text="n.exact ? `bij: ${n.exact}` : undefined"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            :text="soortLabel(n.soort)"
            :supporting-text="n.auteur || undefined"
            horizontal-alignment="right"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell :text="n.open ? 'open' : 'afgehandeld'" horizontal-alignment="right"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="24"></nldd-spacer>
    </nldd-simple-section>

    <!-- Per wet, zodat je ziet waar het werk zit. -->
    <nldd-simple-section width="800px">
      <nldd-title size="6"><h3>Per wet</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="simple" arrow-navigation>
        <nldd-list-item
          v-for="wet in rapport.perWet"
          :key="wet.lawId"
          size="md"
          button
          data-test="wet"
          :href="hrefVoor(wet.lawId, null)"
        >
          <nldd-text-cell
            :text="naamVoor(wet.lawId)"
            :supporting-text="`${wet.open} van ${wet.n} open`"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>

      <nldd-spacer size="24"></nldd-spacer>
    </nldd-simple-section>
  </template>
</template>
