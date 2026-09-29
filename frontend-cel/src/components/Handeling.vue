<script setup>
// Een handeling in een zaak: haar formulier, een proef en vastleggen. Wat
// het formulier vraagt, komt van de runtime (uit de wet: de oordelen, wat de
// stage vraagt, of de velden van het event), niet uit code. Een proef legt
// niets vast: zij zegt welke uitkomsten de engine geeft, of wat er nog mist,
// en per parameter waar hij vandaan kwam. Vastleggen doet de cel; weigert
// zij, dan blijft de kroniek zoals hij was. Zegt de proef om de inhoud nee
// (`reportable`), dan doet het proces de handeling niet uit zichzelf; is het
// feit toch gebeurd, dan meldt de behandelaar het en legt de cel het vast.
import { computed, inject, ref } from 'vue';
import Invoer from './Invoer.vue';
import { bronStatusTekst, herkomstRijen, routesUit, soortVan, uitkomstTekst } from '../tekst.js';
import { naarFormulier, naarWet, veldLabel } from '../formulier.js';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';

const props = defineProps({
  wortel: { type: String, required: true },
  // Zoals de zaak haar beschrijft: name, label, kind, form, trial ...
  handeling: { type: Object, required: true },
});
const emit = defineEmits(['vastgelegd']);
const api = inject('api');
const voorbeelden = inject('voorbeelden');
// Het voorbeeldformulier van deze handeling, of null.
const voorbeeld = computed(() => voorbeelden.value.actions?.[props.handeling.name] ?? null);

// Een bedrag is een getal in het formulier; in welke eenheid, zegt de
// regeling (zie formulier.js).
const invoersoort = (v) => (v.type === 'amount' ? 'number' : v.type);

const waarden = ref(Object.fromEntries(props.handeling.form.map((v) => [v.name, null])));
const proef = ref(props.handeling.trial?.error ? null : props.handeling.trial);
const genomen = ref(null);
const fout = ref(props.handeling.trial?.error ?? '');
const bezig = ref('');

const groepen = computed(() => {
  const uit = [];
  for (const v of props.handeling.form) {
    const titel = v.group ?? (v.kind === 'fact' ? 'Wat er gebeurde' : '');
    let g = uit.find((x) => x.titel === titel);
    if (!g) uit.push((g = { titel, velden: [] }));
    g.velden.push(v);
  }
  return uit;
});

function zet(naam, waarde) {
  waarden.value = { ...waarden.value, [naam]: waarde === '' ? null : waarde };
}

// Wat is ingevuld, met bedragen in de eenheid van de wet.
function formulier(bron) {
  const uit = {};
  for (const v of props.handeling.form) {
    const w = bron[v.name];
    if (w === null || w === undefined) continue;
    uit[v.name] = naarWet(v, w);
  }
  return uit;
}

// Een nldd-dropdown werkt zijn getoonde waarde alleen bij na een keuze van de
// gebruiker; na het invullen met het voorbeeld bouwt de sleutel het formulier
// opnieuw op.
const versie = ref(0);

// Het voorbeeld in de eenheden van het formulier.
function voorbeeldWaarden() {
  return Object.fromEntries(
    props.handeling.form.map((v) => [v.name, naarFormulier(v, voorbeeld.value?.[v.name] ?? null)]),
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
    proef.value = await api.proefhandeling(props.wortel, props.handeling.name, formulier(waarden.value));
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
      props.wortel,
      props.handeling.name,
      formulier(metVoorbeeld ? voorbeeldWaarden() : waarden.value),
      gebeurd,
    );
    genomen.value = uitslag;
    proef.value = uitslag.trial;
    emit('vastgelegd', uitslag);
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = '';
  }
}

const uitkomsten = computed(() =>
  Object.entries({ ...(proef.value?.outputs ?? {}), ...(proef.value?.assessments ?? {}) }).map(([naam, w]) => ({
    naam,
    waarde: uitkomstTekst(w, proef.value?.types?.[naam]),
  })),
);
const herkomst = computed(() =>
  herkomstRijen(proef.value?.parameters, proef.value?.provenance, routesUit(proef.value)),
);
const nietGeleverd = computed(() => proef.value?.not_delivered ?? []);
// De soort komt als {kind, ...} (bij een vervolg met de handeling van het
// besluit erbij).
const soort = computed(() => soortVan(props.handeling));
const soortTekst = computed(() => {
  const h = props.handeling;
  if (soort.value === 'decision') return `besluit, stage ${h.stage}`;
  if (soort.value === 'follow_up') return `stage ${h.stage} van het besluit (${h.kind.decision})`;
  return 'feit uit het verloop van de zaak';
});
</script>

<template>
  <nldd-title size="3">
    <h2>{{ handeling.label }}</h2>
    <span slot="subtitle">{{ soortTekst }}; {{ handeling.article }}</span>
  </nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <template v-if="fout">
    <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-spacer size="8"></nldd-spacer>
  </template>
  <template v-if="!handeling.available && !genomen">
    <nldd-inline-dialog text="Niet in deze stand van de zaak" :supporting-text="handeling.reason"></nldd-inline-dialog>
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
          :key="v.name"
          :label="veldLabel(v)"
          :supporting-label="v.name"
        >
          <Invoer :soort="invoersoort(v)" :label="v.label" :model-value="waarden[v.name]" @update:model-value="zet(v.name, $event)" />
        </nldd-form-field>
      </template>
      <nldd-form-actions>
        <nldd-button variant="primary" type="submit" text="Op proef" :loading="bezig === 'proef' || undefined"></nldd-button>
        <nldd-button
          variant="secondary"
          type="button"
          text="Vastleggen"
          :loading="bezig === 'vastleggen' || undefined"
          :disabled="(genomen !== null && soort !== 'fact') || undefined"
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
        {{ proef.article }}, peildatum {{ proef.reference_date }} ({{ proef.reference_date_from }}).{{ genomen ? '' : ' Er is niets vastgelegd.' }}
      </span>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-container layout="row" gap="8" vertical-alignment="center">
      <nldd-inline-dialog
        :variant="proef.takeable ? 'success' : 'alert'"
        :text="proef.takeable ? 'Te nemen' : 'Niet te nemen'"
        :supporting-text="proef.reason"
      ></nldd-inline-dialog>
      <TraceKnop v-if="proef.trace_text" :trace-text="proef.trace_text" :titel="proef.article" />
    </nldd-container>
    <template v-if="proef.reportable && !genomen">
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
    <template v-for="r in proef.rows ?? []" :key="r.parameter">
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
        <nldd-table-row v-for="b in r.sources" :key="b.cell + b.lexostatus">
          <nldd-text-cell :text="b.cell" :supporting-text="b.transport"></nldd-text-cell>
          <nldd-text-cell
            :text="b.lexostatus"
            :supporting-text="b.reduction ? `reductie via ${b.reduction === 'engine' ? 'de engine' : b.reduction}` : undefined"
          ></nldd-text-cell>
          <nldd-text-cell :text="String(b.queried)"></nldd-text-cell>
          <nldd-text-cell :text="bronStatusTekst(b.status)" :supporting-text="b.error"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <template v-if="r.missing?.length">
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-inline-dialog
          icon="warning"
          icon-color="warning"
          :text="`Kolommen zonder waarde: ${r.missing.join(', ')}`"
          supporting-text="Er wordt niets aangevuld."
        ></nldd-inline-dialog>
      </template>
    </template>
    <template v-for="b in proef.sources" :key="b.cell + b.lexostatus">
      <nldd-inline-dialog
        v-if="b.status !== 'queried'"
        variant="alert"
        :text="`Bron ${b.cell}: ${bronStatusTekst(b.status)}`"
        :supporting-text="b.error"
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
        <nldd-table-row v-for="n in nietGeleverd" :key="n.name">
          <nldd-text-cell :text="n.name" :supporting-text="n.type"></nldd-text-cell>
          <nldd-text-cell :text="n.article"></nldd-text-cell>
          <nldd-text-cell :text="n.description ?? ''"></nldd-text-cell>
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
      <span slot="subtitle">{{ genomen.gram.type }}{{ genomen.gram.stage ? `, stage ${genomen.gram.stage}` : '' }}, op {{ genomen.gram.effective_at }}</span>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-inline-dialog variant="success" text="De cel heeft het gram vastgelegd" :supporting-text="genomen.gram.name"></nldd-inline-dialog>
    <nldd-inline-dialog
      v-for="w in genomen.warnings ?? []"
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
