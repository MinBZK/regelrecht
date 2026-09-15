<script setup>
import { computed, ref, watch } from 'vue';
import { formatMoment, formatValue, humanize } from '../world/format.js';
import { fieldValue } from '../world/events.js';
import { cells, describeParam, lexostatusDefinitions, lexostatusParams, readLexostatus } from '../world/snapshot.js';

// Een cel een vraag stellen: wat stelt u onder deze naam vast, op dit moment?
//
// Alleen lezen, en het antwoord is van de cel. "In deze cel is hierover niets
// vastgesteld" is een gewoon antwoord en geen fout — het staat hier dus als
// antwoord, met de reden die de cel gaf, en niet als melding dat er iets stuk is.
//
// Het **formulier komt uit de definitie** en niet uit deze code: welke parameters
// een lexostatus verlangt en van welk type staat in het beeld, want de cel
// accepteert precies dat en niets anders. De toelichting van de definitie staat
// erboven, zodat te lezen is waar de naam over gaat voordat iemand iets invult.
//
// Eén parameter verdient meer dan haar naam: de **sleutel** van een
// kroniekfilter. Dat is waaronder de cel haar vastleggingen groepeert, en bij
// een kroniek met beschikkingen is dat het zaakkenmerk — een samengestelde
// waarde die uit het sjabloon van een besluit ontstaat en die nergens te zien
// was. Bij zo'n veld staat dus de kroniek en de vorm, en het biedt de waarden
// aan die er nu in die kroniek liggen. Vrije invoer blijft: een vraag over een
// zaak die er nog niet is, is een geldige vraag met "niets vastgesteld" als
// antwoord, en dat is precies wat deze opstelling wil laten zien.
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

/** Alleen cellen die iets publiceren zijn te bevragen. */
const publishers = computed(() =>
  cells(props.snapshot).filter((cell) => lexostatusDefinitions(cell).length > 0),
);

const cellId = ref('');
const name = ref('');
/** Wat de bezoeker per parameter invulde, op naam. */
const values = ref({});
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

/** De gekozen cel, zoals het beeld haar geeft. */
const cell = computed(() => publishers.value.find((candidate) => candidate.id === cellId.value) ?? null);

/** De definities die deze cel publiceert. */
const definitions = computed(() => lexostatusDefinitions(cell.value));

watch(
  definitions,
  (list) => {
    if (!list.some((definition) => definition.name === name.value)) name.value = list[0]?.name ?? '';
  },
  { immediate: true },
);

/** De gekozen definitie: haar toelichting, haar parameters, haar uitkomsten. */
const definition = computed(() => definitions.value.find((candidate) => candidate.name === name.value) ?? null);

/** De parameters van die definitie, met wat er per veld bij hoort te staan. */
const params = computed(() => lexostatusParams(cell.value, definition.value));

// Een andere naam is een ander formulier: wat er voor de vorige vraag ingevuld
// stond, hoort dan niet stil mee te gaan. Een parameter die in beide voorkomt
// blijft wel staan — dezelfde zaak twee keer intypen is geen werk dat iemand
// bedoeld heeft.
watch(params, (list) => {
  const kept = {};
  for (const param of list) if (param.name in values.value) kept[param.name] = values.value[param.name];
  values.value = kept;
});

function setParam(param, event) {
  values.value = { ...values.value, [param.name]: fieldValue(event, values.value[param.name] ?? '') };
}

/** Ligt het gekozen moment ná de klok? Dan valt er niets te vragen. */
const momentAfterClock = computed(() => Boolean(moment.value) && Boolean(clock.value) && moment.value > clock.value);

/** De vraag stellen. Het verkeer loopt via de meegegeven `ask`; hier komt het antwoord. */
async function submit() {
  if (!cellId.value || !name.value || momentAfterClock.value) return;
  asking.value = true;
  answer.value = null;
  try {
    // Een leeg veld gaat niet mee: dan weigert de cel de vraag met "parameter
    // ontbreekt", en dat is de melding die erbij hoort — niet een lege tekst die
    // nergens op slaat.
    const given = Object.fromEntries(
      params.value
        .map((param) => [param.name, values.value[param.name] ?? ''])
        .filter(([, value]) => value !== '' && value !== null && value !== undefined),
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
              <option v-for="option in publishers" :key="option.id" :value="option.id" :selected="option.id === cellId">
                {{ option.id }}
              </option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <nldd-form-field label="Lexostatus">
          <nldd-dropdown @change="name = fieldValue($event, name)">
            <select>
              <option
                v-for="option in definitions"
                :key="option.name"
                :value="option.name"
                :selected="option.name === name"
              >
                {{ option.name }}
              </option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <!-- Waar deze naam over gaat, in de woorden van het wereldbestand. Wat
             de cel erover zegt en niet wat deze app ervan maakt. -->
        <nldd-inline-dialog
          v-if="definition?.doc"
          horizontal-alignment="left"
          icon="info"
          :text="definition.name"
          :supporting-text="definition.doc"
        ></nldd-inline-dialog>

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

        <!-- Eén veld per gedocumenteerde parameter: de cel accepteert precies
             deze namen, dus ze hoeven niet geraden te worden. -->
        <nldd-form-field
          v-for="param in params"
          :key="param.name"
          :label="param.name"
          :supporting-label="describeParam(param)"
        >
          <!-- De sleutel van een kroniek waarin al iets ligt: de bekende
               waarden als keuze, met vrije invoer erbij (`allow-custom`). Een
               vraag over een zaak die er nog niet is, is een geldige vraag. -->
          <nldd-combo-box
            v-if="param.known.length > 0"
            width="full"
            allow-custom
            :value="values[param.name] ?? ''"
            :accessible-label="`Waarde van ${param.name}`"
            :placeholder="param.patterns[0] ?? ''"
            @change="setParam(param, $event)"
            @input="setParam(param, $event)"
          >
            <nldd-menu>
              <nldd-menu-item
                v-for="option in param.known"
                :key="option"
                :text="option"
                :value="option"
              ></nldd-menu-item>
            </nldd-menu>
          </nldd-combo-box>
          <!-- Een datum krijgt het datumveld, net als "Spoel vooruit tot" en de
               formulieren van de acties: Nederlandse notatie in beeld, ISO op de
               draad. Een getal of een ja/nee gaat als tekst mee en wordt door de
               server omgezet naar het type dat de definitie documenteert; komt
               het daar niet doorheen, dan zegt de cel zelf wat er mis is. -->
          <nldd-date-field
            v-else-if="param.type === 'date'"
            width="full"
            :value="values[param.name] ?? ''"
            :accessible-label="`Waarde van ${param.name}`"
            @input="setParam(param, $event)"
            @change="setParam(param, $event)"
          ></nldd-date-field>
          <nldd-text-field
            v-else
            :value="values[param.name] ?? ''"
            :accessible-label="`Waarde van ${param.name}`"
            :placeholder="param.patterns[0] ?? ''"
            @input="setParam(param, $event)"
          ></nldd-text-field>
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
