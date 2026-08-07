<script setup>
/**
 * Geïsoleerde Analyse-preview: zonder backend, DB, auth of traject.
 *
 * Twee bronnen, en de echte staat voorop:
 *
 * - **Repo-corpus**: `corpus/regulation/**` en `corpus/annotations/**` van
 *   deze repo, door de échte WASM-engine gehaald. Dit is de meting die telt.
 * - **Verzonnen**: een fixture die de waarschuwingstoestanden toont die het
 *   repo-corpus (hopelijk) niet heeft: losgeraakte noten, een niet-eenduidige
 *   selector, een tag buiten het vocabulaire, en een wet die de engine niet
 *   kon laden. Zonder deze knop is die helft van de pagina niet te beoordelen.
 */
import { ref, computed, onMounted } from 'vue';
import ModelDefecten from '../components/ModelDefecten.vue';
import NotenRapport from '../components/NotenRapport.vue';
import { aggregeer } from '../lib/notenanalyse.js';
import { laadEchtCorpus, laadVocabulaire, laadMetrieken, hermeet } from './corpus-loader.js';
import { graafUitMetrieken } from '../lib/graafUitMetrieken.js';
import MetriekenRij from '../components/MetriekenRij.vue';
import CorpusGraafView from '../components/CorpusGraafView.vue';

const VERZONNEN_VOCABULAIRE = [
  { id: 'open-norm-not-filled', label: 'Open norm, nog niet ingevuld' },
  { id: 'open-norm-partial', label: 'Open norm, deels ingevuld' },
  { id: 'needs-uitvoeringsbeleid', label: 'Behoeft uitleg door uitvoeringsbeleid' },
  { id: 'missing-document', label: 'Document ontbreekt' },
  { id: 'conflicting-interpretation', label: 'Tegenstrijdige interpretatie' },
];

function noot({ motivation, workflow, tags = [], artikel, exact }) {
  const body = [{ type: 'TextualBody', value: 'toelichting', purpose: 'commenting' }];
  for (const t of tags) body.push({ type: 'TextualBody', value: t, purpose: 'tagging' });
  return {
    type: 'Annotation',
    motivation,
    workflow,
    target: {
      source: 'regelrecht://voorbeeldwet',
      selector: { type: 'TextQuoteSelector', exact, prefix: '', suffix: '', hint: { article_number: artikel } },
    },
    body,
  };
}

// Casus-agnostisch (bouwplan §7): verzonnen namen, geen bedragen, geen
// sector- of organisatietermen.
const VERZONNEN = [
  {
    lawId: 'wet_op_de_voorbeeldregeling',
    notes: [
      noot({ motivation: 'questioning', artikel: '2', exact: 'naar redelijkheid', tags: ['open-norm-not-filled'] }),
      noot({ motivation: 'questioning', artikel: '4', exact: 'bij regeling te bepalen', tags: ['needs-uitvoeringsbeleid'] }),
      noot({ motivation: 'commenting', workflow: 'resolved', artikel: '4', exact: 'de aanvrager' }),
      noot({ motivation: 'linking', workflow: 'resolved', artikel: '7', exact: 'het vastgestelde bedrag' }),
    ],
    ankers: [{ status: 'found' }, { status: 'found' }, { status: 'found' }, { status: 'found' }],
  },
  {
    lawId: 'besluit_uitvoering_voorbeeldregeling',
    notes: [
      noot({ motivation: 'assessing', artikel: '3', exact: 'binnen zes weken', tags: ['conflicting-interpretation'] }),
      noot({ motivation: 'questioning', artikel: '10', exact: 'een inmiddels vervallen zinsnede' }),
      noot({ motivation: 'commenting', artikel: '2', exact: 'meervoudig aangehaalde term' }),
      noot({ motivation: 'questioning', artikel: '5', exact: 'nader te bepalen', tags: ['nog-niet-in-het-vocabulaire'] }),
    ],
    ankers: [{ status: 'found' }, { status: 'orphaned' }, { status: 'ambiguous' }, { status: 'found' }],
  },
  {
    lawId: 'regeling_zonder_geladen_tekst',
    notes: [noot({ motivation: 'questioning', artikel: '1', exact: 'onbekend', tags: ['missing-document'] })],
    ankers: null,
  },
];

// Vandaag als standaard, net als in het product. Het rapport blijft
// deterministisch gegeven een datum; alleen de keuze begint bij vandaag.
const peildatum = ref(new Date().toISOString().slice(0, 10));

const bron = ref('echt'); // 'echt' | 'verzonnen'
const laden = ref(true);
const echt = ref({ perWet: [], wettenInCorpus: 0, diagnostiek: null });
const echtVocabulaire = ref([]);
const laadfout = ref(null);

// Het metriekenrapport uit de WASM-engine (bouwplan §3.1). Los van `echt`
// hierboven, dat de notitielaag (§3.2) draagt: die twee komen uit verschillende
// bronnen en het rapport hoort zichtbaar te maken welke welke is.
const metrieken = ref(null);
const geweigerd = ref([]);
const metriekenFout = ref(null);

onMounted(async () => {
  try {
    echtVocabulaire.value = laadVocabulaire();
    const [noten, gemeten] = await Promise.all([laadEchtCorpus(), laadMetrieken(peildatum.value)]);
    echt.value = noten;
    metrieken.value = gemeten.rapport;
    geweigerd.value = gemeten.geweigerd;
    metriekenFout.value = gemeten.fout;
  } catch (e) {
    laadfout.value = e;
  } finally {
    laden.value = false;
  }
});


const rapport = computed(() =>
  bron.value === 'echt'
    ? aggregeer(echt.value.perWet, echtVocabulaire.value, { wettenInCorpus: echt.value.wettenInCorpus })
    : aggregeer(VERZONNEN, VERZONNEN_VOCABULAIRE, { wettenInCorpus: VERZONNEN.length }),
);

const diagnostiek = computed(() => {
  const d = echt.value.diagnostiek;
  if (bron.value !== 'echt' || !d) return null;
  const delen = [
    `${d.wettenInCorpus} wetten in corpus/regulation`,
    `${d.sidecars} sidecar(s)`,
    `${d.geladenInEngine} door de engine geladen`,
  ];
  if (d.zonderWet.length) delen.push(`sidecar zonder wet: ${d.zonderWet.join(', ')}`);
  if (d.engineFouten.length) delen.push(`engine: ${d.engineFouten.join(' · ')}`);
  return delen.join(' · ');
});

// De graaf draait alleen op het echte corpus: een verzonnen graaf zegt niets
// over samenhang die er werkelijk is.
const graaf = computed(() =>
  bron.value === 'echt' && metrieken.value ? graafUitMetrieken(metrieken.value) : null,
);
const gekozenKnoop = ref(null);

// De preview heeft geen router; `undefined` maakt een rij niet-klikbaar in
// plaats van naar een kapotte URL te wijzen.
/** Herbereken op een nieuwe datum, zonder het corpus opnieuw in te lezen. */
async function zetPeildatum(datum) {
  if (!datum) return;
  peildatum.value = datum;
  try {
    metrieken.value = await hermeet(datum);
  } catch (e) {
    metriekenFout.value = String(e.message ?? e);
  }
}

/** Regelingen die op de peildatum geen geldende versie hebben. */
const nietGeldend = computed(() => metrieken.value?.not_in_force ?? []);

const hrefVoor = () => undefined;
const naamVoor = (lawId) => lawId.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase());
</script>

<template>
  <nldd-app-view>
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" text="Analyse, preview"></nldd-top-title-bar>

      <!-- De ingang zoals hij in de editor staat. Dit blok is een kopie van de
           traject-sidebar uit LibraryView.vue, met dezelfde componenten en
           dezelfde volgorde, zodat te zien is waar Analyse landt zonder dat
           er een backend en een traject nodig zijn. Het is een weergave, geen
           werkende navigatie: de rijen doen niets. -->
      <nldd-simple-section width="800px">
        <nldd-title size="6"><h3>Waar dit in de editor zit</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-inline-dialog
          text="De traject-sidebar, met de vlag panel.analyse aan"
          supporting-text="Staat die vlag uit (de standaard), dan is de vierde rij er niet en is de editor onveranderd. Klikken navigeert naar een eigen pagina; Analyse is geen paneel binnen deze sidebar."
        ></nldd-inline-dialog>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-list variant="simple">
          <nldd-list-item size="md">
            <nldd-icon-cell size="20"><nldd-icon name="gear"></nldd-icon></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell text="Instellingen"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
          <nldd-list-item size="md">
            <nldd-icon-cell size="20"><nldd-icon name="documents"></nldd-icon></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell text="Werkdocumenten"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
          <nldd-list-item size="md">
            <nldd-icon-cell size="20"><nldd-icon name="tasks"></nldd-icon></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell text="Taken"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
          <nldd-list-item size="md" selected>
            <nldd-icon-cell size="20"><nldd-icon name="analytics"></nldd-icon></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell text="Analyse"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-spacer size="24"></nldd-spacer>
      </nldd-simple-section>

      <nldd-simple-section width="800px">
        <nldd-title size="3"><h3>Analyse</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>

        <nldd-inline-dialog
          v-if="bron === 'echt'"
          text="Het corpus van deze repo, door de echte engine"
          :supporting-text="diagnostiek || 'Laden…'"
        ></nldd-inline-dialog>
        <nldd-inline-dialog
          v-else
          variant="alert"
          text="Verzonnen data"
          supporting-text="Toont de waarschuwingstoestanden die het repo-corpus niet heeft. Deze cijfers betekenen niets."
        ></nldd-inline-dialog>

        <nldd-spacer size="12"></nldd-spacer>
        <nldd-button
          size="sm"
          variant="secondary"
          :text="bron === 'echt' ? 'Toon de verzonnen fixture' : 'Terug naar het repo-corpus'"
          @click="bron = bron === 'echt' ? 'verzonnen' : 'echt'"
        ></nldd-button>

        <template v-if="laadfout">
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-banner
            variant="warning"
            text="Corpus niet geladen"
            :supporting-text="laadfout.message"
          ></nldd-banner>
        </template>

        <nldd-spacer size="16"></nldd-spacer>
        <nldd-activity-indicator
          v-if="laden && bron === 'echt'"
          text="Corpus lezen en notities koppelen"
          show-text
        ></nldd-activity-indicator>
      </nldd-simple-section>

      <nldd-simple-section v-if="bron === 'echt' && metrieken" width="800px">
        <nldd-form-field label="Peildatum" for="preview-peildatum">
          <nldd-date-field
            id="preview-peildatum"
            size="sm"
            :value="peildatum"
            @change="zetPeildatum($event.detail.value)"
          ></nldd-date-field>
          <span slot="help-text">
            Bepaalt welke versie van elke regeling meetelt. De cijfers hieronder gelden op deze datum.
          </span>
        </nldd-form-field>
        <template v-if="nietGeldend.length">
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-inline-dialog
            :text="nietGeldend.length === 1 ? 'Eén regeling geldt niet op deze datum' : `${nietGeldend.length} regelingen gelden niet op deze datum`"
            :supporting-text="nietGeldend.map((r) => `${naamVoor(r.law_id)}: ${r.reason === 'not-yet-in-force' ? 'nog niet in werking' : r.ended_on ? `vervallen per ${r.ended_on}` : 'niet gevonden'}`).join(' · ')"
          ></nldd-inline-dialog>
        </template>
        <nldd-spacer size="16"></nldd-spacer>
      </nldd-simple-section>

      <MetriekenRij v-if="bron === 'echt' && metrieken" :rapport="metrieken" :geweigerd="geweigerd" />

      <nldd-simple-section v-if="graaf" width="1100px">
        <nldd-title size="6"><h3>Afhankelijkheidsgraaf</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-inline-dialog
          :text="`${graaf.knopen.length} regelingen, ${graaf.randen.filter((r) => r.van !== r.naar).length} bindingen`"
          supporting-text="Doorgetrokken pijl = input.source (de engine haalt een waarde op). Stippel = implements (een open term wordt ingevuld). Een source onder parameters staat er bewust NIET in: die binding bestaat bij uitvoering niet."
        ></nldd-inline-dialog>
        <nldd-spacer size="12"></nldd-spacer>
        <CorpusGraafView :graaf="graaf" :naam-voor="naamVoor" @knoop="gekozenKnoop = $event" />
        <template v-if="gekozenKnoop">
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-inline-dialog
            :text="naamVoor(gekozenKnoop.lawId)"
            :supporting-text="`${gekozenKnoop.laag ?? 'laag onbekend'} · ${gekozenKnoop.artikelen} artikelen · ${gekozenKnoop.outputs} outputs · ${gekozenKnoop.inkomend} inkomende binding(en)${gekozenKnoop.nietAangeroepen ? ' · niet aangeroepen' : ''}`"
          ></nldd-inline-dialog>
        </template>
        <nldd-spacer size="24"></nldd-spacer>
      </nldd-simple-section>

      <ModelDefecten
        v-if="metrieken"
        :defecten="metrieken.findings"
        :naam-voor="naamVoor"
        :href-voor="hrefVoor"
      />

      <NotenRapport
        v-if="!(laden && bron === 'echt')"
        :rapport="rapport"
        :naam-voor="naamVoor"
        :href-voor="hrefVoor"
      />
    </nldd-page>
  </nldd-app-view>
</template>
