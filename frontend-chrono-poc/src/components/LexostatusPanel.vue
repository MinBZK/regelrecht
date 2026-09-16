<script setup>
import { computed, ref, watch } from 'vue';
import { formatMoment, formatValue, humanize } from '../world/format.js';
import { fieldValue } from '../world/events.js';
import { cells, gramKind, readLexostatus } from '../world/snapshot.js';

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
// nu", maar "wat wist deze cel op T". Het veld volgt de klok zolang de bezoeker
// zelf niets koos, en komt er nooit voorbij — ná de klok heeft niets vastgelegd,
// dus een antwoord "op" zo'n moment zou een voorspelling zijn die zich voordoet
// als een reductie. De server bewaakt dezelfde grens (409); dit veld is de
// vriendelijke kant ervan.

const props = defineProps({
  /** Het beeld van de wereld. */
  snapshot: { type: Object, default: null },
  /** De vraag stellen: `(cel, naam, params, opMoment) => antwoord | null`. */
  ask: { type: Function, required: true },
});

/**
 * Een gram aanwijzen dat de cel gelezen heeft.
 *
 * Dit paneel toont de verwijzing en niet het gram zelf: het gram staat in het
 * tabblad Grammen, met al zijn velden en hun herkomst, en dat hier herhalen zou
 * er een tweede weergave van maken. Wie het kent, kent het al — de pagina brengt
 * de bezoeker erheen. Dezelfde keuze als tussen de tijdlijn en het journaal.
 */
const emit = defineEmits(['show-gram']);

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

/**
 * Het moment dat de bezoeker zelf koos; leeg zolang hij niets koos.
 *
 * Het onderscheid tussen "gekozen" en "niet gekozen" is wat het veld eerlijk
 * houdt. Wie niets kiest, vraagt naar nu — en "nu" verschuift met de klok. Stond
 * de laatste klokstand als waarde in het veld, dan zou vooruitspoelen de vraag
 * stil op de oude klok laten staan: hetzelfde formulier, hetzelfde knopje, een
 * antwoord over gisteren. Dat is precies het soort verkeerd antwoord dat niemand
 * opmerkt.
 */
const chosen = ref('');

/** Het moment waarop de vraag gaat: wat de bezoeker koos, anders de klok. */
const moment = computed(() => chosen.value || clock.value || '');

watch(clock, (value) => {
  // Een klok die terugloopt bestaat niet, maar `POST /api/reset` zet hem wel
  // terug op de startdag, en een keuze van ná die dag zou dan stil een 409
  // opleveren. De keuze vervalt dan en het veld volgt de klok weer. Blijft de
  // keuze vóór de klok, dan blijft ze staan: vooruitspoelen maakt een eerder
  // moment niet ongeldig, en juist dat eerdere moment is waar de bezoeker
  // naar keek.
  if (value && chosen.value > value) chosen.value = '';
});

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

/**
 * Staat de uitleg open?
 *
 * Dicht bij een nieuw antwoord: wie een vraag stelt, wil eerst de uitkomst zien.
 * De uitleg staat eronder voor wie hem wil nalopen, en blijft open zolang de
 * bezoeker hem openhoudt.
 */
const explanationOpen = ref(false);

/** Hoe de cel aan dit antwoord kwam; `null` als het antwoord het niet zegt. */
const explanation = computed(() => answer.value?.reductie ?? null);

/**
 * De uitleg open- of dichtdoen.
 *
 * Een klik ín de uitklap telt niet mee: die bubbelt over de rij heen, en de rij
 * is zelf de knop. Dezelfde afweging als in het grammen- en het journaalpaneel.
 */
function toggleExplanation(event) {
  if (event?.target?.closest?.('nldd-list-item[slot="children"]')) return;
  explanationOpen.value = !explanationOpen.value;
}

/** Wat er in die stroom lag en toch niet meedeed, als één regel. */
const missed = computed(() => {
  const gemist = explanation.value?.gemist;
  if (!gemist) return '';
  return [
    `${gemist.in_de_stroom} gram(men) in deze kroniek`,
    `${gemist.na_het_moment} van ná dit moment`,
    `${gemist.andere_sleutel} over een ander onderwerp`,
    `${gemist.buiten_de_voorwaarden} buiten de voorwaarden`,
  ].join(' · ');
});

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
  explanationOpen.value = false;
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
            @input="chosen = fieldValue($event, moment)"
            @change="chosen = fieldValue($event, moment)"
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

      <!-- Hoe de cel eraan kwam. Onder het antwoord en dichtgeklapt: de uitkomst
           is waar de vraag over ging, de herkomst is waarmee je haar naloopt.
           Staat er geen uitleg bij het antwoord, dan staat hier niets — een lege
           uitklap belooft iets wat er niet is. -->
      <nldd-list
        v-if="explanation"
        type="tree"
        variant="box-tinted"
        accessible-label="Hoe dit antwoord tot stand kwam"
      >
        <nldd-list-item
          size="sm"
          button
          :expanded="explanationOpen || undefined"
          @click="toggleExplanation($event)"
        >
          <nldd-icon-cell icon="search" size="16" color="secondary"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            size="sm"
            min-width="200px"
            text="Zo is dit vastgesteld"
            :supporting-text="explanation.zin"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>

          <!-- De gebruikte grammen. Knoppen: ze brengen de bezoeker naar het
               gram zoals het in het tabblad Grammen staat. -->
          <nldd-list-item
            v-for="gram in explanationOpen ? explanation.grams : []"
            :key="gram.id"
            slot="children"
            size="sm"
            button
            @click="emit('show-gram', gram.id)"
          >
            <nldd-spacer-cell size="20"></nldd-spacer-cell>
            <nldd-icon-cell :icon="gramKind(gram.kind).icon" size="16" color="secondary"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell size="sm" width="104px" :text="formatMoment(gram.opMoment)"></nldd-text-cell>
            <nldd-cell width="132px">
              <nldd-tag size="sm" :color="gramKind(gram.kind).color" :text="gramKind(gram.kind).label"></nldd-tag>
            </nldd-cell>
            <nldd-text-cell
              size="sm"
              min-width="160px"
              :text="gram.name"
              :supporting-text="`${gram.cell} · ${gram.chronicle} · nr. ${gram.volgnummer}`"
            ></nldd-text-cell>
            <!-- Wat dit gram aan een som bijdroeg, of onder welke versie van de
                 regeling het besloten is; allebei alleen waar ze bestaan. -->
            <nldd-text-cell
              v-if="gram.bijdrage !== null"
              size="sm"
              width="fit-content"
              horizontal-alignment="right"
              :text="formatValue(gram.bijdrage)"
            ></nldd-text-cell>
            <nldd-text-cell
              v-else-if="gram.regulationValidFrom"
              size="sm"
              width="fit-content"
              color="secondary"
              :text="`recht van ${formatMoment(gram.regulationValidFrom)}`"
            ></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
          </nldd-list-item>

          <!-- Bij een wetsvorm: waar elke input vandaan kwam. Een input die uit
               een eigen kroniek kwam, draagt het gram dat hem droeg en brengt de
               bezoeker erheen; een berekende input of een parameter ligt nergens
               in een kroniek, en dan is er niets om heen te gaan. -->
          <nldd-list-item
            v-for="input in explanationOpen ? explanation.inputs : []"
            :key="`input-${input.name}`"
            slot="children"
            size="sm"
            :button="Boolean(input.gram) || undefined"
            @click="input.gram && emit('show-gram', input.gram)"
          >
            <nldd-spacer-cell size="20"></nldd-spacer-cell>
            <nldd-icon-cell icon="arrow-right" size="16" color="secondary"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell
              size="sm"
              min-width="200px"
              :text="humanize(input.name)"
              :supporting-text="input.zin"
            ></nldd-text-cell>
            <template v-if="input.gram">
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            </template>
          </nldd-list-item>

          <!-- Niets gelezen: dan is wat er wél lag het enige wat er te melden
               valt. -->
          <nldd-list-item
            v-if="explanationOpen && explanation.grams.length === 0"
            slot="children"
            size="sm"
          >
            <nldd-spacer-cell size="20"></nldd-spacer-cell>
            <nldd-icon-cell icon="info" size="16" color="secondary"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell
              size="sm"
              min-width="200px"
              text="Geen gram gelezen"
              :supporting-text="missed"
            ></nldd-text-cell>
          </nldd-list-item>
        </nldd-list-item>
      </nldd-list>
    </template>
  </nldd-container>
</template>
