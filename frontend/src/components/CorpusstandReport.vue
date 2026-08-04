<script setup>
/**
 * CorpusstandReport — de presentatie van het corpusstand-rapport.
 *
 * Bevat geen fetch en geen router: het rapport komt als prop binnen en de
 * link per rij wordt door de aanroeper gemaakt. Daardoor is dit component
 * zonder backend te previewen (`corpusstand-preview.html`) en zonder mocks te
 * testen, terwijl `CorpusstandView.vue` het echte werk aanlevert.
 */
import { computed } from 'vue';

const props = defineProps({
  /** Uitvoer van `aggregeer()` uit `lib/corpusstand.js`. */
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
    { label: 'Noten in dit traject', waarde: r.totaal, onder: dekking },
    { label: 'Nog open', waarde: r.open, onder: `${r.totaal - r.open} afgehandeld` },
  ];
});

function rijTitel(item) {
  const naam = props.naamVoor(item.lawId);
  return item.artikel ? `${naam} · artikel ${item.artikel}` : naam;
}
</script>

<template>
  <nldd-simple-section v-if="rapport.totaal === 0" width="800px">
    <nldd-inline-dialog
      text="Nog geen notities in dit traject"
      supporting-text="Zodra er noten bij artikelen staan, verschijnen ze hier gegroepeerd."
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

    <!-- Soort en ambiguïteit naast elkaar: twee assen over dezelfde noten,
         geen opeenvolgende stappen. -->
    <nldd-one-half-one-half-section>
      <nldd-title slot="header" size="6"><h3>Waar de noten over gaan</h3></nldd-title>

      <nldd-card slot="left">
        <nldd-container padding="16">
          <nldd-title size="3">
            Naar soort
            <span slot="subtitle">motivation (RFC-005)</span>
          </nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-list variant="simple">
            <nldd-list-item v-for="rij in rapport.naarSoort" :key="rij.key" data-test="soort">
              <nldd-text-cell :text="rij.key"></nldd-text-cell>
              <nldd-text-cell :text="String(rij.n)" horizontal-alignment="right"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-card>

      <nldd-card slot="right">
        <nldd-container padding="16">
          <nldd-title size="3">
            Naar ambiguïteit
            <span slot="subtitle">tags uit ambiguity.yaml</span>
          </nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-inline-dialog
            v-if="rapport.naarTag.length === 0"
            text="Geen ambiguïteit-tags"
            supporting-text="Noten over open normen kunnen een tag dragen die zegt waarom ze nog niet uitvoerbaar zijn."
          ></nldd-inline-dialog>
          <nldd-list v-else variant="simple">
            <nldd-list-item v-for="tag in rapport.naarTag" :key="tag.id" data-test="tag">
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

    <!-- Ankerfouten: het enige blok dat om actie vraagt. Een noot die
         losgeraakt is van zijn tekst wijst meestal op een wetswijziging die
         nog niet is nagelopen. -->
    <nldd-simple-section width="800px">
      <nldd-title size="6"><h3>Ankerfouten</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>

      <nldd-inline-dialog
        v-if="rapport.ankerfouten.items.length === 0 && rapport.ongemeten.length === 0"
        text="Alle noten vinden hun tekst"
        supporting-text="Elke selector is eenduidig terug te vinden in de wettekst waar hij aan hangt."
      ></nldd-inline-dialog>

      <template v-else>
        <nldd-banner
          v-if="rapport.ankerfouten.items.length"
          variant="warning"
          :text="`${rapport.ankerfouten.orphaned} losgeraakt, ${rapport.ankerfouten.ambiguous} niet eenduidig`"
          supporting-text="De aangehaalde tekst is niet meer of niet eenduidig in de wet te vinden. Meestal is de wettekst gewijzigd nadat de noot geschreven werd."
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
      <!-- Eerlijk zijn over wat er nog niet staat: dit is de verklaarde laag
           (bouwplan §3.2). De afgeleide cijfers komen uit het model van de
           engine en volgen wanneer de rekenkern er is. -->
      <nldd-inline-dialog
        text="Dit is de verklaarde laag"
        supporting-text="Dekking, cross-law-integriteit, open terms en untranslatables worden uit het model van de engine berekend en staan er nog niet bij."
      ></nldd-inline-dialog>
      <nldd-spacer size="24"></nldd-spacer>
    </nldd-simple-section>
  </template>
</template>
