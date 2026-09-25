<script setup>
// Een handeling in een zaak: haar formulier, een proef en vastleggen. Wat
// het formulier vraagt, komt van de runtime (uit de wet: de oordelen, wat de
// stage vraagt, of de velden van het event), niet uit code. Een proef legt
// niets vast: zij zegt welke uitkomsten de engine geeft, of wat er nog mist,
// en per parameter waar hij vandaan kwam. Vastleggen doet de cel; weigert
// zij, dan blijft de kroniek zoals hij was. Zegt de proef om de inhoud nee
// (`te_melden`), dan doet het proces de handeling niet uit zichzelf; is het
// feit toch gebeurd, dan meldt de behandelaar het en legt de cel het vast.
import { computed, inject, ref } from 'vue';
import Invoer from './Invoer.vue';
import { herkomstRijen, soortVan, uitkomstTekst } from '../tekst.js';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';

const props = defineProps({
  zaakkenmerk: { type: String, required: true },
  // Zoals de zaak haar beschrijft: naam, label, soort, formulier, proef ...
  handeling: { type: Object, required: true },
});
const emit = defineEmits(['vastgelegd']);
const api = inject('api');
const voorbeelden = inject('voorbeelden');
// Het voorbeeldformulier van deze handeling, of null.
const voorbeeld = computed(() => voorbeelden.value.handelingen?.[props.handeling.naam] ?? null);

// Een bedrag vraagt het formulier in euro; de wet rekent in eurocent.
const bedrag = (v) => v.type === 'bedrag';
const invoersoort = (v) => (bedrag(v) ? 'getal' : v.type);

const waarden = ref(Object.fromEntries(props.handeling.formulier.map((v) => [v.naam, null])));
const proef = ref(props.handeling.proef?.fout ? null : props.handeling.proef);
const genomen = ref(null);
const fout = ref(props.handeling.proef?.fout ?? '');
const bezig = ref('');

const groepen = computed(() => {
  const uit = [];
  for (const v of props.handeling.formulier) {
    const titel = v.groep ?? (v.soort === 'feit' ? 'Wat er gebeurde' : '');
    let g = uit.find((x) => x.titel === titel);
    if (!g) uit.push((g = { titel, velden: [] }));
    g.velden.push(v);
  }
  return uit;
});

function zet(naam, waarde) {
  waarden.value = { ...waarden.value, [naam]: waarde === '' ? null : waarde };
}

// Wat is ingevuld, met bedragen in eurocent.
function formulier(bron) {
  const uit = {};
  for (const v of props.handeling.formulier) {
    const w = bron[v.naam];
    if (w === null || w === undefined) continue;
    uit[v.naam] = bedrag(v) && typeof w === 'number' ? Math.round(w * 100) : w;
  }
  return uit;
}

// Een nldd-dropdown werkt zijn getoonde waarde alleen bij na een keuze van de
// gebruiker; na het invullen met het voorbeeld bouwt de sleutel het formulier
// opnieuw op.
const versie = ref(0);

// Het voorbeeld in de eenheden van het formulier (een bedrag in euro).
function voorbeeldWaarden() {
  return Object.fromEntries(
    props.handeling.formulier.map((v) => {
      const w = voorbeeld.value?.[v.naam] ?? null;
      return [v.naam, bedrag(v) && typeof w === 'number' ? w / 100 : w];
    }),
  );
}

function voorbeeldInvullen() {
  waarden.value = voorbeeldWaarden();
  proef.value = null;
  versie.value++;
}

async function opProef() {
  fout.value = '';
  bezig.value = 'proef';
  try {
    proef.value = await api.proefhandeling(props.zaakkenmerk, props.handeling.naam, formulier(waarden.value));
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = '';
  }
}

async function vastleggen(metVoorbeeld = false, gebeurd = false) {
  fout.value = '';
  bezig.value = metVoorbeeld ? 'voorbeeld' : gebeurd ? 'melden' : 'vastleggen';
  try {
    const uitslag = await api.handeling(
      props.zaakkenmerk,
      props.handeling.naam,
      formulier(metVoorbeeld ? voorbeeldWaarden() : waarden.value),
      gebeurd,
    );
    genomen.value = uitslag;
    proef.value = uitslag.proef;
    emit('vastgelegd', uitslag);
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = '';
  }
}

const uitkomsten = computed(() =>
  Object.entries({ ...(proef.value?.uitkomsten ?? {}), ...(proef.value?.toetsen ?? {}) }).map(([naam, w]) => ({
    naam,
    waarde: uitkomstTekst(naam, w),
  })),
);
const herkomst = computed(() => herkomstRijen(proef.value?.parameters, proef.value?.herkomst));
const nietGeleverd = computed(() => proef.value?.niet_geleverd ?? []);
// De soort komt als {soort, ...} (bij een vervolg met de handeling van het
// besluit erbij).
const soort = computed(() => soortVan(props.handeling));
const soortTekst = computed(() => {
  const h = props.handeling;
  if (soort.value === 'besluit') return `besluit, stage ${h.stage}`;
  if (soort.value === 'vervolg') return `stage ${h.stage} van het besluit (${h.soort.besluit})`;
  return 'feit uit het verloop van de zaak';
});
</script>

<template>
  <nldd-title size="3">
    <h2>{{ handeling.label }}</h2>
    <span slot="subtitle">{{ soortTekst }}; {{ handeling.artikel }}</span>
  </nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-spacer size="8"></nldd-spacer>
  </template>
  <template v-if="!handeling.beschikbaar && !genomen">
    <nldd-inline-dialog text="Niet in deze stand van de zaak" :supporting-text="handeling.reden"></nldd-inline-dialog>
  </template>
  <template v-else>
    <template v-if="voorbeeld && genomen === null">
      <nldd-button-group orientation="horizontal">
        <nldd-button variant="secondary" text="Voorbeeld invullen" @click="voorbeeldInvullen"></nldd-button>
        <nldd-button
          variant="secondary"
          text="Direct vastleggen met voorbeeld"
          :loading="bezig === 'voorbeeld' || undefined"
          @click="vastleggen(true)"
        ></nldd-button>
      </nldd-button-group>
      <nldd-spacer size="16"></nldd-spacer>
    </template>
    <nldd-form :key="versie" novalidate @submit.prevent="opProef">
      <template v-for="g in groepen" :key="g.titel">
        <nldd-form-section v-if="g.titel" :text="g.titel"></nldd-form-section>
        <nldd-form-field
          v-for="v in g.velden"
          :key="v.naam"
          :label="bedrag(v) ? `${v.label} (euro)` : v.label"
          :supporting-label="v.naam"
        >
          <Invoer :soort="invoersoort(v)" :label="v.label" :model-value="waarden[v.naam]" @update:model-value="zet(v.naam, $event)" />
        </nldd-form-field>
      </template>
      <nldd-form-actions>
        <nldd-button variant="primary" type="submit" text="Op proef" :loading="bezig === 'proef' || undefined"></nldd-button>
        <nldd-button
          variant="secondary"
          type="button"
          text="Vastleggen"
          :loading="bezig === 'vastleggen' || undefined"
          :disabled="(genomen !== null && soort !== 'feit') || undefined"
          @click="vastleggen()"
        ></nldd-button>
      </nldd-form-actions>
    </nldd-form>
  </template>

  <template v-if="proef">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="4">
      <h3>{{ genomen ? 'Uitgerekend bij het vastleggen' : 'Op proef' }}</h3>
      <span slot="subtitle">
        {{ proef.artikel }}, peildatum {{ proef.peildatum }} ({{ proef.peildatum_uit }}).{{ genomen ? '' : ' Er is niets vastgelegd.' }}
      </span>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-container layout="row" gap="8" vertical-alignment="center">
      <nldd-inline-dialog
        :variant="proef.te_nemen ? 'success' : 'alert'"
        :text="proef.te_nemen ? 'Te nemen' : 'Niet te nemen'"
        :supporting-text="proef.reden"
      ></nldd-inline-dialog>
      <TraceKnop v-if="proef.trace_text" :trace-text="proef.trace_text" :titel="proef.artikel" />
    </nldd-container>
    <template v-if="proef.te_melden && !genomen">
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-inline-dialog
        icon="info"
        text="Het proces doet dit niet uit zichzelf"
        supporting-text="Is het toch gebeurd, meld het dan: de cel legt het vast, en de zaak toont de gevolgen."
      ></nldd-inline-dialog>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-button
        variant="secondary"
        text="Het is gebeurd: vastleggen"
        :loading="bezig === 'melden' || undefined"
        @click="vastleggen(false, true)"
      ></nldd-button>
    </template>
    <template v-if="uitkomsten.length">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-table columns="minmax(240px,1fr) minmax(160px,1fr)" accessible-label="Uitkomsten">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Uitkomst"></nldd-text-cell>
          <nldd-text-cell text="Waarde"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="u in uitkomsten" :key="u.naam">
          <nldd-text-cell :text="u.naam"></nldd-text-cell>
          <nldd-text-cell :text="u.waarde"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
    </template>
    <template v-for="r in proef.rijen ?? []" :key="r.parameter">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="4"><h3>Samengesteld per regel: {{ r.parameter }}</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(200px,1fr) minmax(200px,1fr) 120px 140px" accessible-label="Bronnen per regel">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Cel"></nldd-text-cell>
          <nldd-text-cell text="Lexostatus"></nldd-text-cell>
          <nldd-text-cell text="Regels"></nldd-text-cell>
          <nldd-text-cell text="Status"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="b in r.bronnen" :key="b.cel + b.lexostatus">
          <nldd-text-cell :text="b.cel" :supporting-text="b.transport"></nldd-text-cell>
          <nldd-text-cell :text="b.lexostatus"></nldd-text-cell>
          <nldd-text-cell :text="String(b.bevraagd)"></nldd-text-cell>
          <nldd-text-cell :text="b.status.replace('_', ' ')" :supporting-text="b.fout"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <template v-if="r.mist?.length">
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-inline-dialog
          icon="warning"
          icon-color="warning"
          :text="`Kolommen zonder waarde: ${r.mist.join(', ')}`"
          supporting-text="Er wordt niets aangevuld."
        ></nldd-inline-dialog>
      </template>
    </template>
    <template v-for="b in proef.bronnen" :key="b.cel + b.lexostatus">
      <nldd-inline-dialog
        v-if="b.status !== 'bevraagd'"
        variant="alert"
        :text="`Bron ${b.cel}: ${b.status.replace('_', ' ')}`"
        :supporting-text="b.fout"
      ></nldd-inline-dialog>
    </template>
    <template v-if="nietGeleverd.length">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="4"><h3>Niet geleverd</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(220px,1fr) minmax(200px,1fr) minmax(300px,2fr)" accessible-label="Niet geleverde parameters">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Parameter"></nldd-text-cell>
          <nldd-text-cell text="Artikel"></nldd-text-cell>
          <nldd-text-cell text="Herkomst volgens het model"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="n in nietGeleverd" :key="n.naam">
          <nldd-text-cell :text="n.naam" :supporting-text="n.type"></nldd-text-cell>
          <nldd-text-cell :text="n.artikel"></nldd-text-cell>
          <nldd-text-cell :text="n.omschrijving ?? ''"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
    </template>
    <template v-if="herkomst.length">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="4"><h3>Herkomst per parameter</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(220px,1fr) 160px minmax(220px,1fr)" accessible-label="Herkomst per parameter">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Parameter"></nldd-text-cell>
          <nldd-text-cell text="Waarde"></nldd-text-cell>
          <nldd-text-cell text="Herkomst"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="h in herkomst" :key="h.naam">
          <nldd-text-cell :text="h.naam"></nldd-text-cell>
          <nldd-text-cell :text="h.waarde"></nldd-text-cell>
          <nldd-text-cell :text="h.bron"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
    </template>
  </template>

  <template v-if="genomen">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="4">
      <h3>Vastgelegd</h3>
      <span slot="subtitle">{{ genomen.gram.type }}{{ genomen.gram.stage ? `, stage ${genomen.gram.stage}` : '' }}, op {{ genomen.gram.op_moment }}</span>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-inline-dialog variant="success" text="De cel heeft het gram vastgelegd" :supporting-text="genomen.gram.name"></nldd-inline-dialog>
    <nldd-inline-dialog
      v-for="w in genomen.waarschuwingen ?? []"
      :key="w"
      icon="warning"
      icon-color="warning"
      text="Waarschuwing"
      :supporting-text="w"
    ></nldd-inline-dialog>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-code-viewer language="yaml" wrap>{{ genomen.yaml }}</nldd-code-viewer>
  </template>
</template>
