<script setup>
import { computed, ref, watch } from 'vue';
import { formatMoment, formatValue, humanize } from '../world/format.js';
import { fieldValue } from '../world/events.js';
import { cells, readLexostatus } from '../world/snapshot.js';

// Een cel een vraag stellen: wat stelt u onder deze naam vast, op dit moment?
//
// Alleen lezen, en het antwoord is van de cel. "In deze cel is hierover niets
// vastgesteld" is een gewoon antwoord en geen fout — het staat hier dus als
// antwoord, met de reden die de cel gaf, en niet als melding dat er iets stuk is.
//
// Welke parameters een lexostatus vraagt staat niet in het beeld; de bezoeker
// geeft ze daarom zelf op, met naam en waarde, zoals de cel ze documenteert.
//
// Het **moment** is wat deze vraag tot tijdreizen maakt: niet "wat weet deze cel
// nu", maar "wat wist deze cel op T". Het veld begint op de klok en komt er nooit
// voorbij — ná de klok heeft niets vastgelegd, dus een antwoord "op" zo'n moment
// zou een voorspelling zijn die zich voordoet als een reductie. De server bewaakt
// dezelfde grens (409); dit veld is de vriendelijke kant ervan.

const props = defineProps({
  /** Het beeld van de wereld. */
  snapshot: { type: Object, default: null },
  /** De vraag stellen: `(cel, naam, params, opMoment) => antwoord | null`. */
  ask: { type: Function, required: true },
});

/** Alleen cellen die iets publiceren zijn te bevragen. */
const publishers = computed(() => cells(props.snapshot).filter((cell) => (cell.lexostatussen ?? []).length > 0));

const cellId = ref('');
const name = ref('');
const params = ref([{ name: '', value: '' }]);
/** Het laatste antwoord, uitgesplitst; `null` zolang er niets gevraagd is. */
const answer = ref(null);
const asking = ref(false);

/** Waar de klok van de wereld staat; de bovengrens van het moment. */
const clock = computed(() => props.snapshot?.clock ?? null);

/** Het moment waarop de vraag gaat. Begint op de klok. */
const moment = ref('');

watch(
  clock,
  (value) => {
    // Leeg, of voorbij de klok: dan is de klok zelf het moment. Een klok die
    // terugloopt bestaat niet, maar `POST /api/reset` zet hem wel terug op de
    // startdag, en een moment van daarna zou dan stil een 409 opleveren.
    // Verder blijft staan wat de bezoeker koos: vooruitspoelen maakt een eerder
    // moment niet ongeldig, en juist dat eerdere moment is waar hij naar keek.
    if (!moment.value || (value && moment.value > value)) moment.value = value ?? '';
  },
  { immediate: true },
);

watch(
  publishers,
  (list) => {
    if (!list.some((cell) => cell.id === cellId.value)) cellId.value = list[0]?.id ?? '';
  },
  { immediate: true },
);

const names = computed(() => publishers.value.find((cell) => cell.id === cellId.value)?.lexostatussen ?? []);

watch(
  names,
  (list) => {
    if (!list.includes(name.value)) name.value = list[0] ?? '';
  },
  { immediate: true },
);

function addParam() {
  params.value = [...params.value, { name: '', value: '' }];
}

function removeParam(index) {
  params.value = params.value.filter((_, position) => position !== index);
}

function setParam(index, key, event) {
  const raw = fieldValue(event, params.value[index][key]);
  params.value = params.value.map((param, position) => (position === index ? { ...param, [key]: raw } : param));
}

/** Ligt het gekozen moment ná de klok? Dan valt er niets te vragen. */
const momentAfterClock = computed(() => Boolean(moment.value) && Boolean(clock.value) && moment.value > clock.value);

/** De vraag stellen. Het verkeer loopt via de meegegeven `ask`; hier komt het antwoord. */
async function submit() {
  if (!cellId.value || !name.value || momentAfterClock.value) return;
  asking.value = true;
  answer.value = null;
  try {
    const given = Object.fromEntries(
      params.value.filter((param) => param.name).map((param) => [param.name, param.value]),
    );
    const payload = await props.ask(cellId.value, name.value, given, moment.value || null);
    answer.value = payload ? readLexostatus(payload) : null;
  } finally {
    asking.value = false;
  }
}
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-title size="5">
      <span>Lexostatus vragen</span>
      <span slot="subtitle">een reductie over de eigen feiten van één cel</span>
    </nldd-title>

    <nldd-form>
      <form @submit.prevent="submit">
        <nldd-form-field label="Cel">
          <nldd-dropdown @change="cellId = fieldValue($event, cellId)">
            <select>
              <option v-for="cell in publishers" :key="cell.id" :value="cell.id" :selected="cell.id === cellId">
                {{ cell.id }}
              </option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <nldd-form-field label="Lexostatus">
          <nldd-dropdown @change="name = fieldValue($event, name)">
            <select>
              <option v-for="option in names" :key="option" :value="option" :selected="option === name">
                {{ option }}
              </option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <!-- Het moment van de vraag. Hetzelfde datumveld als "Spoel vooruit
             tot" op de tijdlijn: Nederlandse notatie in beeld, ISO op de draad. -->
        <nldd-form-field
          label="Op moment"
          supporting-label="dd-mm-jjjj · standaard de klok, nooit erna"
        >
          <nldd-date-field
            :value="moment"
            :max="clock || undefined"
            :invalid="momentAfterClock || undefined"
            @input="moment = fieldValue($event, moment)"
            @change="moment = fieldValue($event, moment)"
          ></nldd-date-field>
          <nldd-form-field-error-text v-if="momentAfterClock">
            De klok staat op {{ formatMoment(clock) }}; over een moment daarna heeft nog niets vastgelegd.
          </nldd-form-field-error-text>
        </nldd-form-field>

        <nldd-form-field
          v-for="(param, index) in params"
          :key="`param-${index}`"
          label="Parameter"
          supporting-label="naam en waarde, zoals de cel ze documenteert"
        >
          <nldd-container layout="row" gap="8" vertical-alignment="center">
            <nldd-text-field
              :value="param.name"
              accessible-label="Naam van de parameter"
              @input="setParam(index, 'name', $event)"
            ></nldd-text-field>
            <nldd-text-field
              :value="param.value"
              accessible-label="Waarde van de parameter"
              @input="setParam(index, 'value', $event)"
            ></nldd-text-field>
            <nldd-icon-button
              size="sm"
              icon="remove"
              text="Parameter weglaten"
              :disabled="params.length === 1 || undefined"
              @click="removeParam(index)"
            ></nldd-icon-button>
          </nldd-container>
        </nldd-form-field>

        <nldd-form-actions>
          <nldd-button-group>
            <nldd-button
              variant="primary"
              type="submit"
              start-icon="search"
              text="Vraag stellen"
              :disabled="!cellId || !name || momentAfterClock || undefined"
              :loading="asking || undefined"
            ></nldd-button>
            <nldd-button variant="secondary" start-icon="add" text="Parameter" @click="addParam"></nldd-button>
          </nldd-button-group>
        </nldd-form-actions>
      </form>
    </nldd-form>

    <nldd-inline-dialog
      v-if="publishers.length === 0"
      icon="radar"
      text="Geen cel publiceert iets"
      supporting-text="Er is nu geen lexostatus om naar te vragen."
    ></nldd-inline-dialog>

    <template v-if="answer">
      <nldd-title v-if="answer.established" size="6">
        <span slot="overline">Antwoord</span>
        <span>{{ answer.cell }} · {{ answer.name }}</span>
        <span slot="subtitle">vastgesteld, geldig op {{ formatMoment(answer.opMoment) }}</span>
      </nldd-title>
      <nldd-list v-if="answer.established" variant="box-tinted" :accessible-label="`Antwoord van cel ${answer.cell}`">
        <nldd-list-item v-for="value in answer.values" :key="value.name" size="sm">
          <nldd-text-cell size="sm" :text="humanize(value.name)"></nldd-text-cell>
          <nldd-text-cell
            size="sm"
            width="fit-content"
            horizontal-alignment="right"
            :text="formatValue(value.value)"
          ></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>

      <!-- Niets vastgesteld is een antwoord, niet een fout: een gewone melding,
           met de reden die de cel gaf. -->
      <nldd-inline-dialog
        v-else
        icon="info"
        text="Niets vastgesteld"
        :supporting-text="answer.reason"
      ></nldd-inline-dialog>
    </template>
  </nldd-container>
</template>
