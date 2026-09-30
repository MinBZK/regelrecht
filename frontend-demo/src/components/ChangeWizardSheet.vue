<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { CHANGE_TYPES, claimsFromAnswers } from '../data/changeWizard.js';
import { formatValue } from '../data/format.js';
import { useDemo } from '../store/demoStore.js';

// Wijziging doorgeven: iemand meldt een verandering in zijn leven (verhuisd,
// ander inkomen, uit elkaar) in plaats van per regeling een waarde te
// corrigeren. Elke melding wordt vertaald naar correcties op de wet die het
// gegeven bezit (zie data/changeWizard.js), en daarna rekent elke regeling die
// ervan afhangt opnieuw.
//
// Drie stappen: wat wilt u doorgeven, wat verandert er, en een bevestiging die
// laat zien wat er precies ingaat en naar welke wet.

const props = defineProps({
  open: { type: Boolean, default: false },
});
const emit = defineEmits(['close', 'submitted']);
const { corpus, submitClaim, profile, subjectBsn, features } = useDemo();

const sheet = ref(null);
const step = ref(0);
const typeId = ref(null);
const answers = ref({});
const reason = ref('');
const error = ref('');
/** Wat er is ingediend, voor de slotstap. */
const submitted = ref(null);

const type = computed(() => CHANGE_TYPES.find((t) => t.id === typeId.value) ?? null);
const claims = computed(() => claimsFromAnswers(type.value, answers.value));
/** De gekozen gebeurtenis bij 'huishouden', ook als die nog niet kan. */
const chosenEvent = computed(() => (type.value?.events ?? []).find((e) => e.value === answers.value.event) ?? null);
const lawName = computed(() => (type.value ? corpus.value?.lawById(type.value.law)?.name ?? type.value.law : null));

function reset() {
  step.value = 0;
  typeId.value = null;
  answers.value = {};
  reason.value = '';
  error.value = '';
  submitted.value = null;
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      reset();
      await nextTick();
      sheet.value?.show?.();
    } else {
      sheet.value?.hide?.();
    }
  },
);

function chooseType(id) {
  typeId.value = id;
  answers.value = {};
  error.value = '';
  step.value = 1;
}

function setAnswer(name, value) {
  answers.value = { ...answers.value, [name]: value };
  error.value = '';
}

function back() {
  error.value = '';
  step.value = Math.max(0, step.value - 1);
}

function toConfirm() {
  if (chosenEvent.value?.unsupported) {
    error.value = chosenEvent.value.unsupported;
    return;
  }
  if (!claims.value.length) {
    error.value = type.value?.events
      ? 'Kies wat er in uw huishouden verandert.'
      : 'Vul ten minste één veld in. Wat u leeg laat, blijft ongewijzigd.';
    return;
  }
  error.value = '';
  step.value = 2;
}

/**
 * De melding indienen: elke vertaalde waarde wordt een correctie op haar wet,
 * met dezelfde weg als een losse correctie op het portaal — een behandelaar
 * beoordeelt haar, tenzij het profiel automatisch goedkeurt.
 */
function submit() {
  const bsn = subjectBsn();
  const list = claims.value;
  for (const c of list) {
    submitClaim({
      lawId: c.law,
      tileLawId: null,
      input: c.input,
      keyField: type.value.keyField,
      keyValue: bsn,
      oldValue: null,
      newValue: c.value,
      reason: reason.value || `Wijziging doorgegeven: ${type.value.label.toLowerCase()}.`,
    });
  }
  submitted.value = { count: list.length, label: type.value.label, law: lawName.value };
  emit('submitted', { type: type.value.id, claims: list });
  step.value = 3;
}

/** Hoe een waarde in de bevestiging leest: een bedrag als bedrag, een adres als regel. */
function show(claim) {
  if (claim.value === null) return 'niet meer van toepassing';
  if (typeof claim.value === 'object') {
    return Object.entries(claim.value)
      .filter(([k]) => k !== 'type')
      .map(([, v]) => v)
      .join(' ');
  }
  if (typeof claim.value === 'number' && claim.value >= 1000) {
    return formatValue(claim.value, { type: 'amount', type_spec: { unit: 'eurocent' } });
  }
  return String(claim.value);
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheet" placement="right" accessible-label="Wijziging doorgeven" @close="emit('close')">
      <nldd-page>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar
            text="Wijziging doorgeven"
            :supporting-text="type ? type.label : 'Geef door wat er in uw situatie is veranderd'"
            dismiss-text="Sluiten"
            @dismiss="emit('close')"
          ></nldd-top-title-bar>
        </nldd-container>

        <nldd-container padding="16" gap="16">
          <!-- Stap 1: wat wilt u doorgeven -->
          <template v-if="step === 0">
            <nldd-rich-text spacing="tight">
              <p>Kies wat er is veranderd. De overheid rekent daarna al uw regelingen opnieuw uit; u hoeft dat niet per regeling door te geven.</p>
            </nldd-rich-text>
            <nldd-list variant="box" accessible-label="Soort wijziging">
              <nldd-list-item v-for="t in CHANGE_TYPES" :key="t.id" size="md" button @click="chooseType(t.id)">
                <nldd-icon-cell :icon="t.icon" size="20" color="accent"></nldd-icon-cell>
                <nldd-spacer-cell size="12"></nldd-spacer-cell>
                <nldd-text-cell :text="t.label" :supporting-text="t.description"></nldd-text-cell>
                <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <!-- Stap 2: wat verandert er -->
          <template v-else-if="step === 1">
            <!-- Een gebeurtenis (huishouden): kiezen, geen invullen. -->
            <template v-if="type?.events">
              <nldd-list variant="box" accessible-label="Wat verandert er">
                <nldd-list-item
                  v-for="e in type.events"
                  :key="e.value"
                  size="md"
                  button
                  :selected="answers.event === e.value || undefined"
                  @click="setAnswer('event', e.value)"
                >
                  <nldd-icon-cell :icon="e.icon" size="20" :color="e.unsupported ? 'secondary' : 'accent'"></nldd-icon-cell>
                  <nldd-spacer-cell size="12"></nldd-spacer-cell>
                  <nldd-text-cell :text="e.label" :supporting-text="e.unsupported ? 'Nog niet mogelijk in deze demo' : undefined"></nldd-text-cell>
                  <nldd-icon-cell v-if="answers.event === e.value && !e.unsupported" icon="checked" size="16" color="success"></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
            </template>

            <!-- Waarden: alleen wat verandert hoeft ingevuld. -->
            <template v-else>
              <nldd-rich-text spacing="tight">
                <p>Vul alleen in wat er verandert. Wat u leeg laat, blijft zoals het geregistreerd staat.</p>
              </nldd-rich-text>
              <template v-for="group in type?.groups ?? []" :key="group.label">
                <nldd-title size="4"><h2>{{ group.label }}</h2></nldd-title>
                <nldd-form-field v-for="f in group.fields" :key="f.name" :label="f.label" optional>
                  <nldd-text-field
                    :value="answers[f.name] ?? ''"
                    width="full"
                    :keyboard="f.kind === 'text' ? undefined : 'numeric'"
                    :prefix="f.scale ? '€' : undefined"
                    @input="setAnswer(f.name, $event.detail?.value ?? $event.target.value)"
                  ></nldd-text-field>
                </nldd-form-field>
              </template>
            </template>

            <nldd-banner v-if="error" variant="critical" :text="error"></nldd-banner>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button variant="primary" text="Verder" @click="toConfirm"></nldd-button>
                <nldd-button variant="secondary" text="Terug" @click="back"></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </template>

          <!-- Stap 3: bevestigen. Wat er ingaat en naar welke wet, voordat het ingaat. -->
          <template v-else-if="step === 2">
            <nldd-rich-text spacing="tight">
              <p>U geeft het volgende door aan <strong>{{ lawName }}</strong>. Controleer het voordat u het indient.</p>
            </nldd-rich-text>
            <nldd-list variant="box-tinted" accessible-label="Wat u doorgeeft">
              <nldd-list-item v-for="c in claims" :key="c.input" size="md">
                <nldd-text-cell :text="c.label" :supporting-text="c.input"></nldd-text-cell>
                <nldd-text-cell width="fit-content" horizontal-alignment="right" :text="show(c)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-form-field label="Toelichting" optional>
              <nldd-multi-line-text-field
                :value="reason"
                rows="3"
                placeholder="Bijvoorbeeld: ik ben per 1 maart verhuisd."
                @input="reason = $event.detail?.value ?? $event.target.value"
              ></nldd-multi-line-text-field>
              <nldd-form-field-help-text>
                {{ features.AUTO_APPROVE_CLAIMS
                  ? 'Uw melding wordt direct gebruikt in de berekening.'
                  : 'Uw regelingen rekenen meteen met wat u doorgeeft; een behandelaar beoordeelt de wijziging voordat de uitkomst vaststaat.' }}
              </nldd-form-field-help-text>
            </nldd-form-field>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button variant="primary" text="Doorgeven" @click="submit"></nldd-button>
                <nldd-button variant="secondary" text="Terug" @click="back"></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </template>

          <!-- Stap 4: wat er nu gebeurt. -->
          <template v-else>
            <nldd-inline-dialog
              icon="checked"
              icon-color="success"
              text="Uw wijziging is doorgegeven"
              :supporting-text="`${submitted?.count === 1 ? 'Eén gegeven is' : `${submitted?.count} gegevens zijn`} gewijzigd bij ${submitted?.law}. Uw regelingen zijn opnieuw berekend; op het portaal ziet u wat dit betekent.`"
            ></nldd-inline-dialog>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button variant="primary" text="Naar mijn regelingen" @click="emit('close')"></nldd-button>
                <nldd-button variant="secondary" text="Nog een wijziging" @click="reset"></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </template>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
