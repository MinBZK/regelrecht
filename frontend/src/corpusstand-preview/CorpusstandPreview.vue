<script setup>
/**
 * Geïsoleerde Corpusstand-preview: zonder backend, DB, auth of traject.
 *
 * Twee bronnen, en de echte staat voorop:
 *
 * - **Repo-corpus** — `corpus/regulation/**` en `corpus/annotations/**` van
 *   deze repo, door de échte WASM-engine gehaald. Dit is de meting die telt.
 * - **Verzonnen** — een fixture die de waarschuwingstoestanden toont die het
 *   repo-corpus (hopelijk) niet heeft: losgeraakte noten, een niet-eenduidige
 *   selector, een tag buiten het vocabulaire, en een wet die de engine niet
 *   kon laden. Zonder deze knop is die helft van de pagina niet te beoordelen.
 */
import { ref, computed, onMounted } from 'vue';
import CorpusstandReport from '../components/CorpusstandReport.vue';
import { aggregeer } from '../lib/corpusstand.js';
import { laadEchtCorpus, laadVocabulaire, laadWettenVoorGraaf } from './corpus-loader.js';
import { bouwGraaf } from '../lib/corpusgraaf.js';
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

const bron = ref('echt'); // 'echt' | 'verzonnen'
const laden = ref(true);
const echt = ref({ perWet: [], wettenInCorpus: 0, diagnostiek: null });
const echtVocabulaire = ref([]);
const laadfout = ref(null);

onMounted(async () => {
  try {
    echtVocabulaire.value = laadVocabulaire();
    echt.value = await laadEchtCorpus();
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
const graaf = computed(() => (bron.value === 'echt' ? bouwGraaf(laadWettenVoorGraaf()) : null));
const gekozenKnoop = ref(null);

// De preview heeft geen router; `undefined` maakt een rij niet-klikbaar in
// plaats van naar een kapotte URL te wijzen.
const hrefVoor = () => undefined;
const naamVoor = (lawId) => lawId.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase());
</script>

<template>
  <nldd-app-view>
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" text="Corpusstand — preview"></nldd-top-title-bar>

      <nldd-simple-section width="800px">
        <nldd-title size="3"><h3>Corpusstand</h3></nldd-title>
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
          text="Corpus lezen en noten resolven"
          show-text
        ></nldd-activity-indicator>
      </nldd-simple-section>

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

      <CorpusstandReport
        v-if="!(laden && bron === 'echt')"
        :rapport="rapport"
        :naam-voor="naamVoor"
        :href-voor="hrefVoor"
      />
    </nldd-page>
  </nldd-app-view>
</template>
