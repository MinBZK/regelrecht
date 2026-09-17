<script setup>
import { computed, ref, watch } from 'vue';
import { formatMoment, formatValue, humanize } from '../world/format.js';
import { fieldValue } from '../world/events.js';
import {
  cells,
  describeParam,
  gramKind,
  lexostatusDefinitions,
  lexostatusParams,
  placeholderFor,
  readLexostatus,
} from '../world/snapshot.js';

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

/**
 * Is deze uitkomst een lijst met regels?
 *
 * Een uitkomst mag een lijst van objecten zijn — de openstaandvorm levert er een
 * met de termijnen waar haar bedragen uit bestaan — en zo'n lijst als één regel
 * tekst tonen zou de uitkomst onleesbaar maken. Wat een lijst van iets ánders is
 * (getallen, teksten) blijft een gewone waarde: daar valt geen tabel van te
 * maken, want er zijn geen kolommen.
 *
 * Een lege lijst telt als lijst met regels. Aan een lege lijst is niet te zien
 * wat ze had kunnen dragen, en "er zijn geen regels" is voor elke lege lijst
 * waar — een lege termijnenlijst is juist het antwoord dat er niets vervallen
 * is, en hoort niet als "0 items" tussen de bedragen te verdwijnen.
 */
function isRowList(value) {
  return (
    Array.isArray(value) &&
    value.every((row) => row !== null && typeof row === 'object' && !Array.isArray(row))
  );
}

/** De losse uitkomsten van het antwoord: alles wat geen lijst met regels is. */
const plainValues = computed(() => (answer.value?.values ?? []).filter((value) => !isRowList(value.value)));

/**
 * De uitkomsten die een tabel zijn, met hun kolommen.
 *
 * De kolommen komen uit de regels zelf en niet uit deze app: wat een regel
 * draagt, zegt de cel, en een vaste kolomlijst hier zou een veld verbergen zodra
 * er een bij komt. De volgorde is die waarin de velden binnenkomen — op naam,
 * want het beeld komt uit een geordende map. Een "mooiere" volgorde hier zou een
 * lijstje veldnamen in deze app zetten, en dan weet de app iets van de casus.
 */
const tableValues = computed(() =>
  (answer.value?.values ?? [])
    .filter((value) => isRowList(value.value))
    .map((value) => {
      const columns = [];
      for (const row of value.value) for (const key of Object.keys(row)) if (!columns.includes(key)) columns.push(key);
      return { name: value.name, columns, rows: value.value };
    }),
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
  explanationOpen.value = false;
  try {
    // Een leeg veld gaat niet mee: dan weigert de cel de vraag met "parameter
    // ontbreekt", en dat is de melding die erbij hoort — niet een lege tekst die
    // nergens op slaat.
    const given = Object.fromEntries(
      params.value
        .map((param) => [param.name, values.value[param.name] ?? ''])
        .filter(([, value]) => value !== ''),
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
            :unmet="momentAfterClock ? 'lexostatus-moment-na-klok' : undefined"
            @input="chosen = fieldValue($event, moment)"
            @change="chosen = fieldValue($event, moment)"
          ></nldd-date-field>
          <nldd-validation-list v-if="momentAfterClock">
            <nldd-validation-item id="lexostatus-moment-na-klok">
              De klok staat op {{ formatMoment(clock) }}; over een moment daarna heeft nog niets vastgelegd.
            </nldd-validation-item>
          </nldd-validation-list>
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
            :placeholder="placeholderFor(param)"
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
            :placeholder="placeholderFor(param)"
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
      <nldd-list
        v-if="answer.established && plainValues.length > 0"
        variant="box-tinted"
        :accessible-label="`Antwoord van cel ${answer.cell}`"
      >
        <nldd-list-item v-for="value in plainValues" :key="value.name" size="sm">
          <nldd-text-cell size="sm" :text="humanize(value.name)"></nldd-text-cell>
          <nldd-text-cell
            size="sm"
            width="fit-content"
            horizontal-alignment="right"
            :text="formatValue(value.value)"
          ></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>

      <!-- Een uitkomst die uit regels bestaat, krijgt een tabel. De openstaandvorm
           levert zo haar termijnen: de bedragen erboven zijn er de optelling van,
           en zonder deze regels is die optelling niet na te lopen. De kolommen
           komen uit de regels zelf — deze app weet niet welke velden een cel
           erin zet. -->
      <template v-for="table in answer.established ? tableValues : []" :key="table.name">
        <nldd-title size="6">
          <span>{{ humanize(table.name) }}</span>
          <span slot="subtitle">waar de bedragen hierboven uit bestaan</span>
        </nldd-title>
        <nldd-table
          v-if="table.columns.length > 0"
          :columns="`repeat(${table.columns.length}, minmax(96px, 1fr))`"
          :accessible-label="`${humanize(table.name)} van cel ${answer.cell}`"
        >
          <nldd-table-row slot="header">
            <nldd-text-cell
              v-for="column in table.columns"
              :key="column"
              size="sm"
              :text="humanize(column)"
            ></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="(row, position) in table.rows" :key="position">
            <nldd-text-cell
              v-for="column in table.columns"
              :key="column"
              size="sm"
              :text="formatValue(row[column])"
            ></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
        <!-- Een lege lijst is een antwoord: er was op dit moment niets om te
             tonen. Een tabel zonder kolommen zou beweren dat er velden zijn. -->
        <nldd-inline-dialog
          v-else
          icon="info"
          text="Geen regels"
          :supporting-text="`Op dit moment draagt '${table.name}' geen enkele regel.`"
        ></nldd-inline-dialog>
      </template>

      <!-- Niets vastgesteld is een antwoord, niet een fout: een gewone melding,
           met de reden die de cel gaf. Een eigen `v-if` en geen `v-else`: tussen
           de uitkomsten en deze melding staan de tabellen, en die breken de
           keten. -->
      <nldd-inline-dialog
        v-if="!answer.established"
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
