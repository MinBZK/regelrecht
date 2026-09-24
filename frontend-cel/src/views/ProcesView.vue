<script setup>
// Een proces. Met de rol aanvrager (een portaal): inloggen met eHerkenning,
// zien wat het beleid aanbiedt en indienen. Met de rol behandelaar: inloggen
// als medewerker, de werkvoorraad, een zaak met een proefbesluit en het
// besluit. Welke rollen er zijn, zegt GET /api/processen. De kroniek en de
// lexostatussen zijn van de cel waarin het proces vastlegt; die komen van
// /cellen/<id>, zonder login.
import { computed, onMounted, provide, ref } from 'vue';
import { celApi, procesApi } from '../api.js';
import InloggenView from './InloggenView.vue';
import MogelijkhedenView from './MogelijkhedenView.vue';
import MedewerkerView from './MedewerkerView.vue';
import AanvraagView from './AanvraagView.vue';
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
// De voorbeelden van het proces (inloggen, aanvraag, besluit); zonder: leeg.
const voorbeelden = ref({ inloggen: [], aanvraag: null, besluit: null });
provide('voorbeelden', voorbeelden);

const rollen = computed(() => ['aanvrager', 'behandelaar'].filter((r) => props.proces.rollen?.[r]));
const rol = ref(rollen.value[0] ?? null);
// Een sessie per proces: wie als de andere rol inlogt, vervangt haar.
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
  if (r === 'aanvrager' && props.proces.portaal) return 'mogelijkheden';
  if (r === 'behandelaar' && props.proces.behandeling) return 'werkvoorraad';
  return 'kroniek';
}

onMounted(async () => {
  if (rol.value === null) return;
  api
    .voorbeelden()
    .then((v) => (voorbeelden.value = v))
    .catch(() => {});
  try {
    if (props.proces.rollen.behandelaar) {
      const m = await api.medewerkerSessie().catch(() => null);
      if (m) {
        sessie.value = { rol: 'behandelaar', naam: m.naam };
        kiesRol('behandelaar');
        return;
      }
    }
    if (props.proces.rollen.aanvrager) {
      const s = await api.sessie().catch(() => null);
      if (s) sessie.value = { rol: 'aanvrager', ...s };
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

function aanvragen({ subsidiejaar }) {
  vooraf.value = { subsidiejaar };
  scherm.value = 'aanvraag';
}

function ingediend(gram) {
  nieuw.value = gram;
  scherm.value = 'kroniek';
}

async function uitloggen() {
  const weg = sessie.value?.rol === 'behandelaar' ? api.medewerkerUitloggen : api.uitloggen;
  await weg().catch(() => {});
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

const wie = computed(() => {
  const s = sessie.value;
  if (!s) return '';
  return s.rol === 'behandelaar' ? `${s.naam}, behandelaar` : `${s.persoon}, KvK ${s.kvk}`;
});
</script>

<template>
  <template v-if="rollen.length > 1">
    <nldd-segmented-control accessible-label="Rol" :value="rol" @change="kiesRol($event.detail.value)">
      <nldd-segmented-control-item value="aanvrager" text="Aanvrager"></nldd-segmented-control-item>
      <nldd-segmented-control-item value="behandelaar" text="Behandelaar"></nldd-segmented-control-item>
    </nldd-segmented-control>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="geladen && rol === 'aanvrager' && !ingelogd">
    <InloggenView @ingelogd="sessie = { rol: 'aanvrager', ...$event }" />
  </template>
  <template v-else-if="geladen && rol === 'behandelaar' && !ingelogd">
    <MedewerkerView @ingelogd="sessie = { rol: 'behandelaar', naam: $event.naam }" />
  </template>
  <template v-else-if="geladen">
    <nldd-container layout="row" horizontal-alignment="space-between" vertical-alignment="center">
      <nldd-tab-bar size="md" accessible-label="Scherm" @tabchange="tab">
        <nldd-tab-bar-item
          v-if="rol === 'aanvrager' && proces.portaal"
          data-scherm="mogelijkheden"
          text="Wat kan ik aanvragen"
          :current="scherm === 'mogelijkheden' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="rol === 'aanvrager' && proces.portaal && mogelijk.length"
          data-scherm="aanvraag"
          text="Indienen"
          :current="scherm === 'aanvraag' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="rol === 'behandelaar' && proces.behandeling"
          data-scherm="werkvoorraad"
          text="Werkvoorraad"
          :current="scherm === 'werkvoorraad' || undefined"
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
    <AanvraagView v-else-if="scherm === 'aanvraag'" :key="vooraf.subsidiejaar" :vooraf="vooraf" @ingediend="ingediend" />
    <template v-else-if="scherm === 'werkvoorraad'">
      <ZaakView v-if="zaak" :key="zaak" :zaakkenmerk="zaak" @terug="zaak = null" />
      <WerkvoorraadView v-else :kolommen="werkvoorraadKolommen" @open="zaak = $event" />
    </template>
    <KroniekView v-else-if="scherm === 'kroniek'" :nieuw="nieuw" :portaal="proces.portaal" />
    <LexostatusView v-else :lexostatussen="cel.lexostatussen" />
  </template>
</template>
