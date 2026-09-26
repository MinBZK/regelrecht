<script setup>
// Het loket: een aanvraag die langs een andere weg binnenkwam (op papier,
// aan de balie), ingevoerd door een medewerker namens de aanvrager. De
// aanvrager duidt het loket aan met de velden van een portaalkanaal (uit de
// configuratie van het proces); niemand logde daarmee in. De dag van
// ontvangst telt rechtens (Awb 4:13: de beslistermijn loopt vanaf de
// ontvangst) en wordt het op_moment van het gram; het invoeren is
// vastgelegd_op. Het proces weigert een dag na vandaag, en een dag vóór de
// openstelling van het tijdvak als het beleid die noemt.
import { computed, inject, ref } from 'vue';
import { veldTekst } from '../formulier.js';
import { lege, portaalkanalen } from '../kanaal.js';
import AanvraagView from './AanvraagView.vue';
import Invoer from '../components/Invoer.vue';

const api = inject('api');
const proces = inject('proces');
const emit = defineEmits(['ingediend']);

const kanalen = computed(() => portaalkanalen(proces));
const kanaalId = ref(kanalen.value[0]?.id ?? null);
const kanaal = computed(() => kanalen.value.find((k) => k.id === kanaalId.value) ?? null);
const aanvrager = ref(lege(kanaal.value));
const ontvangenOp = ref(null);

function kiesKanaal(id) {
  kanaalId.value = id;
  aanvrager.value = lege(kanaal.value);
}

function verstuur(external) {
  return api.loketIndienen({
    aanvrager: { kanaal: kanaalId.value, ...aanvrager.value },
    ontvangen_op: ontvangenOp.value ?? '',
    external,
  });
}
</script>

<template>
  <nldd-rich-text>
    <p>
      Voer een aanvraag in die langs een andere weg binnenkwam, namens de aanvrager. De dag van ontvangst is de
      aanvraagdatum; het invoeren wordt apart vastgelegd.
    </p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <AanvraagView :verstuur="verstuur" :toetsen="false" titel="Aanvraag invoeren aan het loket" @ingediend="emit('ingediend', $event)">
    <template #voor>
      <nldd-form-section text="Aanvrager en ontvangst"></nldd-form-section>
      <nldd-form-field v-if="kanalen.length > 1" label="Aanvrager aangeduid met">
        <nldd-segmented-control accessible-label="Kanaal van de aanvrager" :value="kanaalId" @change="kiesKanaal($event.detail.value)">
          <nldd-segmented-control-item v-for="k in kanalen" :key="k.id" :value="k.id" :text="k.rol"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-form-field>
      <nldd-form-field v-for="v in kanaal?.velden ?? []" :key="kanaalId + v.naam" :label="v.label">
        <nldd-text-field
          :value="aanvrager[v.naam]"
          :name="`aanvrager-${v.naam}`"
          :keyboard="v.numeriek ? 'numeric' : undefined"
          @input="aanvrager = { ...aanvrager, [v.naam]: veldTekst($event) }"
        ></nldd-text-field>
      </nldd-form-field>
      <nldd-form-field label="Ontvangen op" supporting-label="de datumstempel">
        <Invoer soort="datum" label="Ontvangen op" :model-value="ontvangenOp" @update:model-value="ontvangenOp = $event" />
      </nldd-form-field>
    </template>
  </AanvraagView>
</template>
