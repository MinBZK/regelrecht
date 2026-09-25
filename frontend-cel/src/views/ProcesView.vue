<script setup>
// Een proces. Welke rollen er zijn, langs welk kanaal ze inloggen en welke
// schermen ze hebben, zegt GET /api/processen (`kanalen` en `rollen` in
// proces.yaml): een rol met routes portaal ziet wat het beleid aanbiedt en
// dient in, een rol met routes behandeling ziet de werkvoorraad, een zaak
// met haar handelingen (het besluit, de bekendmaking, een betaling, een
// feit uit het verloop), een rol met routes loket voert een
// aanvraag in die langs een andere weg binnenkwam. De kroniek en de
// lexostatussen zijn van de cel waarin het proces vastlegt; die komen van
// /cellen/<id>, zonder login.
import { computed, onMounted, provide, ref } from 'vue';
import { celApi, procesApi } from '../api.js';
import { beginscherm as beginVan, rollenVan, sessieTekst } from '../kanaal.js';
import InloggenView from './InloggenView.vue';
import MogelijkhedenView from './MogelijkhedenView.vue';
import AanvraagView from './AanvraagView.vue';
import LoketView from './LoketView.vue';
import KroniekView from './KroniekView.vue';
import LexostatusView from './LexostatusView.vue';
import WerkvoorraadView from './WerkvoorraadView.vue';
import ZaakView from './ZaakView.vue';

const props = defineProps({
  proces: { type: Object, required: true },
  // De cel waarin het proces vastlegt, zoals GET /api/cellen haar beschrijft.
  cel: { type: Object, required: true },
});

const api = procesApi(props.proces.id);
provide('api', api);
provide('celApi', celApi(props.cel.id));
// De voorbeelden van het proces (inloggen, aanvraag, en per handeling een
// formulier); zonder: leeg.
const voorbeelden = ref({ inloggen: [], aanvraag: null, handelingen: {} });
provide('voorbeelden', voorbeelden);
provide('proces', props.proces);

const rollen = computed(() => rollenVan(props.proces));
const rol = ref(rollen.value[0]?.id ?? null);
// Een sessie per proces: wie in een andere rol inlogt, vervangt haar.
const sessie = ref(null);
const geladen = ref(rol.value === null);
const scherm = ref(beginscherm(rol.value));
const nieuw = ref(null);
const zaak = ref(null);
// De aanvraagmogelijkheden die de wet deze organisatie geeft. Het tabblad
// Indienen verschijnt alleen als er een is; dit is aanbieden, geen
// afscherming: het proces weigert een indiening niet.
const mogelijk = ref([]);
const vooraf = ref({});

function beginscherm(r) {
  return beginVan(props.proces, r);
}

// De routegroepen van de gekozen rol.
const mag = (routes) => props.proces.rollen?.[rol.value]?.routes?.includes(routes) ?? false;

// Het kanaal van de gekozen rol, en of de login de rol moet noemen (als er
// langs dat kanaal meer dan een rol inlogt).
const kanaalId = computed(() => props.proces.rollen?.[rol.value]?.kanaal ?? null);
const kanaal = computed(() => props.proces.kanalen?.[kanaalId.value] ?? null);
const rolMeesturen = computed(() => rollen.value.filter((r) => r.kanaal === kanaalId.value).length > 1);

onMounted(async () => {
  if (rol.value === null) return;
  api
    .voorbeelden()
    .then((v) => (voorbeelden.value = v))
    .catch(() => {});
  try {
    const s = await api.sessie().catch(() => null);
    if (s && props.proces.rollen?.[s.rol]) {
      sessie.value = s;
      kiesRol(s.rol);
    }
  } finally {
    geladen.value = true;
  }
});

function kiesRol(r) {
  rol.value = r;
  scherm.value = beginscherm(r);
  zaak.value = null;
}

const ingelogd = computed(() => sessie.value !== null && sessie.value.rol === rol.value);
const werkvoorraadKolommen = computed(
  () => props.cel.lexostatussen.find((l) => l.name === props.proces.behandeling?.werkvoorraad)?.kolommen ?? [],
);

function mogelijkhedenGeladen(lijst) {
  mogelijk.value = lijst.filter((m) => m.oordeel === 'mogelijk');
}

// Wat vooraf vaststaat: het veld van het gekozen tijdvak.
function aanvragen(velden) {
  vooraf.value = velden;
  scherm.value = 'aanvraag';
}

function ingediend(gram) {
  nieuw.value = gram;
  scherm.value = 'kroniek';
}

async function uitloggen() {
  await api.uitloggen(sessie.value.kanaal).catch(() => {});
  sessie.value = null;
  mogelijk.value = [];
  vooraf.value = {};
  scherm.value = beginscherm(rol.value);
}

function tab(e) {
  const naar = e.detail?.item?.dataset?.scherm;
  if (naar) {
    scherm.value = naar;
    zaak.value = null;
  }
}

const wie = computed(() => sessieTekst(props.proces, sessie.value));
</script>

<template>
  <template v-if="rollen.length > 1">
    <nldd-segmented-control accessible-label="Rol" :value="rol" @change="kiesRol($event.detail.value)">
      <nldd-segmented-control-item v-for="r in rollen" :key="r.id" :value="r.id" :text="r.label"></nldd-segmented-control-item>
    </nldd-segmented-control>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="geladen && rol !== null && !ingelogd && kanaal">
    <InloggenView
      :key="rol"
      :kanaal-id="kanaalId"
      :kanaal="kanaal"
      :rol="rol"
      :rol-meesturen="rolMeesturen"
      @ingelogd="sessie = $event"
    />
  </template>
  <template v-else-if="geladen">
    <nldd-container layout="row" horizontal-alignment="space-between" vertical-alignment="center">
      <nldd-tab-bar size="md" accessible-label="Scherm" @tabchange="tab">
        <nldd-tab-bar-item
          v-if="mag('portaal') && proces.portaal"
          data-scherm="mogelijkheden"
          text="Wat kan ik aanvragen"
          :current="scherm === 'mogelijkheden' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="mag('portaal') && proces.portaal && mogelijk.length"
          data-scherm="aanvraag"
          text="Indienen"
          :current="scherm === 'aanvraag' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="mag('behandeling') && proces.behandeling"
          data-scherm="werkvoorraad"
          text="Werkvoorraad"
          :current="scherm === 'werkvoorraad' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="mag('loket') && proces.loket"
          data-scherm="loket"
          text="Loket"
          :current="scherm === 'loket' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item data-scherm="kroniek" text="Kroniek" :current="scherm === 'kroniek' || undefined"></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="cel.lexostatussen.length"
          data-scherm="lexostatus"
          text="Lexostatus"
          :current="scherm === 'lexostatus' || undefined"
        ></nldd-tab-bar-item>
      </nldd-tab-bar>
      <nldd-button
        v-if="sessie"
        variant="neutral-transparent"
        start-icon="logout"
        :text="`Uitloggen (${wie})`"
        @click="uitloggen"
      ></nldd-button>
    </nldd-container>
    <nldd-spacer size="24"></nldd-spacer>
    <MogelijkhedenView v-if="scherm === 'mogelijkheden'" @geladen="mogelijkhedenGeladen" @aanvragen="aanvragen" />
    <AanvraagView v-else-if="scherm === 'aanvraag'" :key="JSON.stringify(vooraf)" :vooraf="vooraf" @ingediend="ingediend" />
    <LoketView v-else-if="scherm === 'loket'" @ingediend="ingediend" />
    <template v-else-if="scherm === 'werkvoorraad'">
      <ZaakView v-if="zaak" :key="zaak" :zaakkenmerk="zaak" @terug="zaak = null" />
      <WerkvoorraadView v-else :kolommen="werkvoorraadKolommen" @open="zaak = $event" />
    </template>
    <KroniekView v-else-if="scherm === 'kroniek'" :nieuw="nieuw" :portaal="proces.portaal" />
    <LexostatusView v-else :lexostatussen="cel.lexostatussen" />
  </template>
</template>
