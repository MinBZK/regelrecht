<script setup>
import { computed, ref, watch } from 'vue';
import { formatMoment, humanize } from '../world/format.js';
import { fieldValue } from '../world/events.js';
import { decidedAlready, describeEffect, gramKind, initialForm, isPrefilled } from '../world/snapshot.js';

// Eén actie uit het wereldbestand: wie haar doet, wat ze uitwerkt, wat de actor
// invult, en of ze nu kan.
//
// Het formulier komt uit `form` in het beeld en niet uit deze code: een veld
// heet zoals de wereld het noemt en heeft het type dat de cel accepteert. Een
// actie die nu niet kan blijft staan met de reden erbij; ze is uit te voeren
// zodra de wereld zegt dat het kan.
//
// Wat de wereld al weet, staat er al in: `prefill` uit het beeld vult de velden
// voordat iemand iets typt, en het veld zegt erbij dat het voorgevuld is. Het
// blijft een voorstel — wie er iets anders van maakt, verstuurt dat, en dan is
// het ook geen voorinvulling meer.
//
// Elk type heeft zijn eigen veld, en een datum dus ook: `nldd-date-field`, net
// als "Spoel vooruit tot" op de tijdlijn. Dat veld toont de Nederlandse notatie
// (dd-mm-jjjj) en geeft ISO terug, wat precies het verschil is dat hier telt —
// een tekstveld liet de invuller zelf gokken welke van de twee de server wilde,
// en de Nederlandse notatie was daarbij de meest voor de hand liggende gok.
//
// Ligt er over deze zaak al een besluit, dan staat dat er ("al besloten op …")
// en vraagt de kaart om bevestiging voordat ze het nog eens doet. De knop blijft
// bruikbaar: een tweede besluit is legitiem — dat is juist het verhaal van deze
// opstelling — maar het legt een tweede decretogram, en dat hoort niet per
// ongeluk te gebeuren.

const props = defineProps({
  /** De actie uit het beeld. */
  action: { type: Object, required: true },
  /** Het beeld van de wereld; waaruit blijkt wat er al besloten is. */
  snapshot: { type: Object, default: null },
  /** Staat er een wijziging onderweg? */
  busy: { type: Boolean, default: false },
  /** Waarom de server deze actie weigerde; `null` zolang er niets misging. */
  error: { type: String, default: null },
});

const emit = defineEmits(['run']);

const values = ref(initialForm(props.action));
/** De velden die nog leeg zijn en bij de laatste poging ingevuld hadden moeten zijn. */
const missing = ref([]);
/** De velden waar de bezoeker zelf iets van maakte; die volgen de wereld niet meer. */
const typed = ref([]);

/** Wat de bezoeker zelf invulde, op naam. */
function typedValues() {
  return Object.fromEntries(typed.value.map((name) => [name, values.value[name]]));
}

// Een nieuw beeld geeft dezelfde actie opnieuw; het formulier hoort dan opnieuw
// te beginnen wanneer de velden zelf veranderen, en anders te blijven staan zoals
// de bezoeker het invulde.
watch(
  () => (props.action.form ?? []).map((field) => `${field.name}:${field.type}`).join(','),
  () => {
    typed.value = [];
    values.value = initialForm(props.action);
    missing.value = [];
  },
);

const fields = computed(() => props.action.form ?? []);
const effect = computed(() => describeEffect(props.action.effect));

/** Het besluit dat er al ligt over de zaak in dit formulier; `null` zo niet. */
const decided = computed(() => decidedAlready(props.snapshot, props.action, values.value));

/** Staat de bevestiging open? Alleen tussen een klik en het antwoord daarop. */
const confirming = ref(false);

// Een ander formulier is een andere zaak: de vraag die over de vorige ging, hoort
// dan niet meer op het scherm te staan.
watch(decided, () => {
  confirming.value = false;
});

/** Wat er al ligt, als tag; `null` zolang er niets ligt. */
const decidedTag = computed(() =>
  decided.value
    ? {
        color: gramKind('decretogram').color,
        icon: gramKind('decretogram').icon,
        text: `al besloten op ${formatMoment(decided.value.opMoment)}`,
      }
    : null,
);

/** Wat de wereld over de actie zegt: kan ze nu? */
const availabilityTag = computed(() => ({
  color: props.action.available ? 'success' : 'warning',
  icon: props.action.available ? 'check-mark-circle' : 'clock',
  text: props.action.available ? 'kan nu' : 'kan nu niet',
}));

/**
 * De tags op de kaart: wat er al ligt gaat voorop.
 *
 * "Al besloten op …" komt in de plaats van "kan nu" — dat er al iets ligt is dan
 * het nieuws, en dat de actie kan spreekt uit de knop eronder. Maar het komt
 * nooit in de plaats van **"kan nu niet"**: een actie die niet kan is iets anders
 * dan een actie die al besloot, en die twee onder één tag schuiven zou de kaart
 * groen laten lijken terwijl de wereld de klik weigert.
 */
const tags = computed(() => {
  const list = decidedTag.value ? [decidedTag.value] : [];
  if (!decidedTag.value || !props.action.available) list.push(availabilityTag.value);
  return list;
});

/** Waarom er bevestigd moet worden, in de woorden van wat er ligt. */
const confirmation = computed(() => {
  if (!decided.value) return '';
  const zaak = decided.value.zaakkenmerk ? ` over zaak '${decided.value.zaakkenmerk}'` : '';
  const ligt = decided.value.count === 1 ? 'ligt er al één besluit' : `liggen er al ${decided.value.count} besluiten`;
  return `In cel '${props.action.effect?.cell}'${zaak} ${ligt}, het laatste van `
    + `${formatMoment(decided.value.opMoment)}. Nog een keer besluiten legt een decretogram erbij; het `
    + 'bestaande blijft staan, want een kroniek wordt nooit overschreven.';
});

// Komt de wereld met een ander voorstel — de klok is doorgelopen, of er ligt nu
// een aanvraag waar er eerst geen was — dan volgen de velden die niemand
// aanraakte dat voorstel. Anders zou het datumveld de klok van bij het laden
// blijven tonen, en zou een `$last` die net gevuld raakte alleen na een verse
// pagina zichtbaar zijn. Wat de bezoeker zelf typte, blijft van hem.
watch(
  () => JSON.stringify(props.action.prefill ?? {}),
  () => {
    values.value = initialForm(props.action, typedValues());
    // Een veld dat de wereld zojuist invulde, staat niet meer leeg; de melding
    // dat het ontbreekt hoort er dan ook niet meer bij te staan.
    missing.value = missing.value.filter((name) => {
      const field = fields.value.find((candidate) => candidate.name === name);
      return field !== undefined && isEmpty(field);
    });
  },
);

function errorId(field) {
  return `${props.action.id}-${field.name}-fout`;
}

function isMissing(field) {
  return missing.value.includes(field.name);
}

/** Staat hier nog wat de wereld voorstelde? */
function wasPrefilled(field) {
  return isPrefilled(props.action, field.name, values.value[field.name]);
}

/**
 * Wat er onder het label van een veld staat: het woord dat de wereld voor het
 * type gebruikt, en bij een datum de notatie erbij. Die notatie hoort in het
 * label en niet als placeholder in het veld — zo staat ze er ook nog terwijl er
 * iets ingevuld is.
 */
function supportingLabel(field) {
  return field.type === 'date' ? `${field.type} · dd-mm-jjjj` : field.type;
}

/** Hetzelfde type, maar dan zoals het in een zin staat. */
function typeName(field) {
  return field.type === 'date' ? 'datum' : field.type;
}

/**
 * Is dit een veld waarin een getal hoort?
 *
 * `amount` net zo goed als `number`: het verschil tussen de twee zit in wát er
 * staat en niet in hoe het ingevuld wordt — een bedrag is een getal. Dezelfde
 * gelijkstelling als die van de server bij het lezen van een ingevulde waarde,
 * zodat een bedrag hier niet als tekst de deur uit gaat.
 */
function isNumeric(field) {
  return field.type === 'number' || field.type === 'amount';
}

/** Leeg is niet ingevuld; `false` bij een ja/nee-veld is wél een antwoord. */
function isEmpty(field) {
  const value = values.value[field.name];
  if (field.type === 'boolean') return false;
  return value === '' || value === null || value === undefined;
}

function setValue(field, event) {
  const raw = fieldValue(event, values.value[field.name]);
  values.value = {
    ...values.value,
    [field.name]: isNumeric(field) ? (raw === '' || raw === null ? null : Number(raw)) : raw,
  };
  markTyped(field);
  missing.value = missing.value.filter((name) => name !== field.name);
}

function setChecked(field, event) {
  values.value = { ...values.value, [field.name]: Boolean(event?.detail?.checked ?? event?.target?.checked) };
  markTyped(field);
}

/** Dit veld is nu van de bezoeker: een later voorstel overschrijft het niet meer. */
function markTyped(field) {
  if (!typed.value.includes(field.name)) typed.value = [...typed.value, field.name];
}

function submit() {
  const incomplete = fields.value.filter(isEmpty).map((field) => field.name);
  missing.value = incomplete;
  if (incomplete.length > 0) return;
  // Ligt er al een besluit, dan is de eerste klik de vraag en de tweede het
  // antwoord; anders gaat de actie meteen.
  if (decided.value) {
    confirming.value = true;
    return;
  }
  run();
}

/** De actie echt uitvoeren. */
function run() {
  confirming.value = false;
  emit('run', { action: props.action, values: values.value });
}
</script>

<template>
  <nldd-card :accessible-label="action.label">
    <nldd-container slot="header" layout="stack" gap="8" padding="16" padding-bottom="8">
      <nldd-title size="6">
        <span slot="overline">{{ action.id }}</span>
        <span>{{ action.label }}</span>
        <span slot="subtitle">{{ effect }}</span>
      </nldd-title>
      <nldd-container layout="wrap" gap="4">
        <nldd-tag
          v-for="tag in tags"
          :key="tag.text"
          size="sm"
          :color="tag.color"
          :icon="tag.icon"
          :text="tag.text"
        ></nldd-tag>
      </nldd-container>
    </nldd-container>

    <nldd-container layout="stack" gap="12" padding="16" padding-top="8">
      <nldd-text v-if="action.doc" size="sm" color="secondary">{{ action.doc }}</nldd-text>

      <nldd-banner
        v-if="!action.available && action.unavailable_reason"
        variant="warning"
        text="Deze actie kan nu niet"
        :supporting-text="action.unavailable_reason"
      ></nldd-banner>

      <!-- De weigering van de server hoort bij het formulier dat haar uitlokte
           en niet bovenaan de pagina: wie een veld verkeerd invult, kijkt naar
           dat veld. De tekst is die van de wereld zelf; deze app verzint er geen
           eigen uitleg bij. -->
      <nldd-banner
        v-if="error"
        variant="critical"
        text="Deze actie is niet uitgevoerd"
        :supporting-text="error"
      ></nldd-banner>

      <!-- Eigen <form> binnen nldd-form: de door het ontwerpsysteem aanbevolen
           modus voor frameworks, zodat de component geen kinderen verplaatst
           die Vue zelf plaatst. -->
      <nldd-form>
        <form @submit.prevent="submit">
          <template v-for="field in fields" :key="field.name">
            <!-- Een ja/nee-veld draagt zijn label zelf; het zou anders twee keer
                 boven hetzelfde veld staan. Een voorgevulde stand staat er
                 zichtbaar in — de schakelaar ís de weergave van haar waarde —
                 dus er hoort geen tweede melding bij. -->
            <nldd-switch-field
              v-if="field.type === 'boolean'"
              :label="humanize(field.name)"
              :checked="values[field.name] || undefined"
              @change="setChecked(field, $event)"
            ></nldd-switch-field>
            <nldd-form-field
              v-else
              :label="humanize(field.name)"
              :supporting-label="supportingLabel(field)"
            >
              <!-- Licht gemarkeerd, en in de woorden van wat het is: een
                   voorstel van de wereld, geen vastgelegd feit. -->
              <nldd-form-field-help-text v-if="wasPrefilled(field)">
                Voorgevuld met wat de wereld al weet; pas het aan als het anders is.
              </nldd-form-field-help-text>
              <nldd-number-field
                v-if="isNumeric(field)"
                :value="values[field.name] ?? undefined"
                width="full"
                :invalid="isMissing(field) || undefined"
                :unmet="isMissing(field) ? errorId(field) : undefined"
                @input="setValue(field, $event)"
                @change="setValue(field, $event)"
              ></nldd-number-field>
              <nldd-date-field
                v-else-if="field.type === 'date'"
                :value="values[field.name] ?? ''"
                width="full"
                :invalid="isMissing(field) || undefined"
                :unmet="isMissing(field) ? errorId(field) : undefined"
                @input="setValue(field, $event)"
                @change="setValue(field, $event)"
              ></nldd-date-field>
              <nldd-text-field
                v-else
                :value="values[field.name] ?? ''"
                :invalid="isMissing(field) || undefined"
                :unmet="isMissing(field) ? errorId(field) : undefined"
                @input="setValue(field, $event)"
                @change="setValue(field, $event)"
              ></nldd-text-field>
              <nldd-validation-list v-if="isMissing(field)">
                <nldd-validation-item :id="errorId(field)">
                  Vul {{ humanize(field.name).toLowerCase() }} in; de cel accepteert alleen een {{ typeName(field) }}.
                </nldd-validation-item>
              </nldd-validation-list>
            </nldd-form-field>
          </template>

          <!-- De bevestiging staat in het formulier en niet erboven: ze gaat
               over deze klik, met de knoppen op de plek waar de vorige stond.
               Een inline dialoog en geen modaal venster — er hoeft niets
               weggeklikt te worden om de kaart te kunnen lezen. -->
          <nldd-inline-dialog
            v-if="confirming && decidedTag"
            horizontal-alignment="left"
            :icon="decidedTag.icon"
            text="Er ligt al een besluit"
            :supporting-text="confirmation"
          >
            <nldd-button
              slot="actions"
              variant="primary"
              start-icon="send"
              text="Toch besluiten"
              :loading="busy || undefined"
              @click="run()"
            ></nldd-button>
            <nldd-button
              slot="actions"
              variant="neutral-tinted"
              text="Annuleren"
              @click="confirming = false"
            ></nldd-button>
          </nldd-inline-dialog>

          <nldd-form-actions v-else>
            <nldd-button-group>
              <nldd-button
                variant="primary"
                type="submit"
                start-icon="send"
                :text="`${action.label} uitvoeren`"
                :loading="busy || undefined"
              ></nldd-button>
            </nldd-button-group>
          </nldd-form-actions>
        </form>
      </nldd-form>
    </nldd-container>
  </nldd-card>
</template>
