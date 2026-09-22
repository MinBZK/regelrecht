<script setup>
// Een cel. Met de rol aanvrager (een portaal): inloggen met eHerkenning,
// indienen, en de eigen kroniek en lexostatus van de ingelogde KvK. Met de rol
// behandelaar: inloggen als medewerker, de werkvoorraad, een zaak met een
// proefbesluit, en de hele kroniek. Zonder rollen: de kroniek en de
// lexostatussen, zonder inloggen. Welke rollen er zijn, zegt GET /api/cellen.
import { computed, onMounted, provide, ref } from 'vue';
import { celApi } from '../api.js';
import InloggenView from './InloggenView.vue';
import MedewerkerView from './MedewerkerView.vue';
import AanvraagView from './AanvraagView.vue';
import KroniekView from './KroniekView.vue';
import LexostatusView from './LexostatusView.vue';
import WerkvoorraadView from './WerkvoorraadView.vue';
import ZaakView from './ZaakView.vue';

const props = defineProps({ cel: { type: Object, required: true } });

const api = celApi(props.cel.id);
provide('api', api);

const rollen = computed(() => ['aanvrager', 'behandelaar'].filter((r) => props.cel.rollen?.[r]));
const rol = ref(rollen.value[0] ?? null);
// Een sessie per cel: wie als de andere rol inlogt, vervangt haar.
const sessie = ref(null);
const geladen = ref(rol.value === null);
const scherm = ref(beginscherm(rol.value));
const nieuw = ref(null);
const zaak = ref(null);

function beginscherm(r) {
  if (r === 'aanvrager' && props.cel.portaal) return 'aanvraag';
  if (r === 'behandelaar' && props.cel.behandeling) return 'werkvoorraad';
  return 'kroniek';
}

onMounted(async () => {
  if (rol.value === null) return;
  try {
    if (props.cel.rollen.behandelaar) {
      const m = await api.medewerkerSessie().catch(() => null);
      if (m) {
        sessie.value = { rol: 'behandelaar', naam: m.naam };
        kiesRol('behandelaar');
        return;
      }
    }
    if (props.cel.rollen.aanvrager) {
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
  () => props.cel.lexostatussen.find((l) => l.name === props.cel.behandeling?.werkvoorraad)?.kolommen ?? [],
);

function ingediend(gram) {
  nieuw.value = gram;
  scherm.value = 'kroniek';
}

async function uitloggen() {
  const weg = sessie.value?.rol === 'behandelaar' ? api.medewerkerUitloggen : api.uitloggen;
  await weg().catch(() => {});
  sessie.value = null;
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
          v-if="rol === 'aanvrager' && cel.portaal"
          data-scherm="aanvraag"
          text="Indienen"
          :current="scherm === 'aanvraag' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="rol === 'behandelaar' && cel.behandeling"
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
    <AanvraagView v-if="scherm === 'aanvraag'" @ingediend="ingediend" />
    <template v-else-if="scherm === 'werkvoorraad'">
      <ZaakView v-if="zaak" :key="zaak" :zaakkenmerk="zaak" @terug="zaak = null" />
      <WerkvoorraadView v-else :kolommen="werkvoorraadKolommen" @open="zaak = $event" />
    </template>
    <KroniekView v-else-if="scherm === 'kroniek'" :nieuw="nieuw" :portaal="cel.portaal" />
    <LexostatusView v-else :lexostatussen="cel.lexostatussen" />
  </template>
</template>
