<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import OrgLogo from './OrgLogo.vue';
import { fieldSpec, formatValue, humanize, isAmountSpec } from '../data/format.js';
import { useDemo } from '../store/demoStore.js';

// The citizen corrects one register value. The correction becomes a claim on
// the law that owns the input; once approved (by the caseworker, or at once
// for a profile with AUTO_APPROVE_CLAIMS) the engine uses it instead of the
// register value. Opens as a sheet, mirrors `open` onto show()/hide().

const props = defineProps({
  open: { type: Boolean, default: false },
  /** Lineage value node: { law, name, value, service, keyField, keyValue } */
  node: { type: Object, default: null },
  /** The tile's law id, so the case for that law knows about this claim. */
  tileLawId: { type: String, default: null },
  /** A value only the citizen can know (no register): applies at once. */
  selfDeclared: { type: Boolean, default: false },
});
const emit = defineEmits(['close', 'submitted']);
const { corpus, submitClaim, profile } = useDemo();

const sheet = ref(null);
const newValue = ref('');
const reason = ref('');
const error = ref('');

const spec = computed(() => (props.node ? fieldSpec(corpus.value?.lawById(props.node.law)?.doc, props.node.name) : null));
const law = computed(() => (props.node ? corpus.value?.lawById(props.node.law) : null));
const kind = computed(() => {
  const s = spec.value;
  if (typeof props.node?.value === 'boolean' || s?.type === 'boolean') return 'boolean';
  if (isAmountSpec(s)) return 'amount';
  if (s?.type === 'number' || typeof props.node?.value === 'number') return 'number';
  if (s?.type === 'date' || /^\d{4}-\d{2}-\d{2}$/.test(String(props.node?.value ?? ''))) return 'date';
  if (props.node && (Array.isArray(props.node.value) || (typeof props.node.value === 'object' && props.node.value !== null))) return 'json';
  return 'text';
});

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      sheet.value?.hide?.();
      return;
    }
    error.value = '';
    reason.value = '';
    const v = props.node?.value;
    if (kind.value === 'amount' && typeof v === 'number') newValue.value = (v / 100).toFixed(2).replace('.', ',');
    else if (kind.value === 'boolean') newValue.value = v ? 'true' : 'false';
    else if (kind.value === 'json') newValue.value = JSON.stringify(v ?? null, null, 2);
    else newValue.value = v === null || v === undefined ? '' : String(v);
    await nextTick();
    sheet.value?.show?.();
  },
  { immediate: true },
);

function parse() {
  const raw = String(newValue.value).trim();
  switch (kind.value) {
    case 'amount': {
      const n = Number(raw.replace(/\./g, '').replace(',', '.'));
      if (!Number.isFinite(n)) throw new Error('Vul een bedrag in, bijvoorbeeld 1.250,00.');
      return Math.round(n * 100);
    }
    case 'number': {
      const n = Number(raw.replace(',', '.'));
      if (!Number.isFinite(n)) throw new Error('Vul een getal in.');
      return n;
    }
    case 'boolean':
      return raw === 'true';
    case 'date':
      if (!/^\d{4}-\d{2}-\d{2}$/.test(raw)) throw new Error('Vul een datum in als JJJJ-MM-DD.');
      return raw;
    case 'json':
      try {
        return JSON.parse(raw);
      } catch {
        throw new Error('Dit is geen geldige JSON.');
      }
    default:
      return raw === '' ? null : raw;
  }
}

function submit() {
  if (!props.node) return;
  let value;
  try {
    value = parse();
  } catch (e) {
    error.value = e.message;
    return;
  }
  if (!reason.value.trim() && !props.selfDeclared) {
    error.value = 'Geef een reden op; die ziet de behandelaar.';
    return;
  }
  const claim = submitClaim({
    lawId: props.node.law,
    tileLawId: props.tileLawId,
    input: props.node.name,
    keyField: props.node.keyField ?? 'bsn',
    keyValue: props.node.keyValue ?? profile.value?.bsn,
    oldValue: props.node.value,
    newValue: value,
    reason: reason.value.trim(),
    selfDeclared: props.selfDeclared,
  });
  emit('submitted', claim);
  emit('close');
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheet" placement="right" width="480px" accessible-label="Gegeven corrigeren" @close="emit('close')">
      <nldd-page v-if="node">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="humanize(node.name)" :supporting-text="law?.name ?? node.law" dismiss-text="Sluiten" @dismiss="emit('close')"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="16">
          <nldd-list variant="box" accessible-label="Huidige waarde">
            <nldd-list-item size="md">
              <nldd-cell v-if="node.service"><OrgLogo :service="node.service" /></nldd-cell>
              <nldd-spacer-cell v-if="node.service" size="12"></nldd-spacer-cell>
              <nldd-text-cell :overline="selfDeclared ? 'Door u op te geven' : 'Geregistreerde waarde'" :text="formatValue(node.value, spec)" :supporting-text="node.service ? `Bron: ${corpus.services[node.service]?.name ?? node.service}` : 'Dit gegeven staat in geen register; u geeft het zelf op.'"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-rich-text v-if="spec?.description"><p>{{ spec.description }}</p></nldd-rich-text>
          <nldd-form-field :label="kind === 'amount' ? 'Nieuwe waarde (in euro)' : 'Nieuwe waarde'">
            <nldd-dropdown v-if="kind === 'boolean'">
              <select :value="newValue" @change="newValue = $event.target.value">
                <option value="true">Ja</option>
                <option value="false">Nee</option>
              </select>
            </nldd-dropdown>
            <nldd-code-editor v-else-if="kind === 'json'" :value="newValue" rows="6" @input="newValue = $event.target.value"></nldd-code-editor>
            <nldd-text-field v-else :value="newValue" :placeholder="kind === 'date' ? 'JJJJ-MM-DD' : ''" @input="newValue = $event.detail?.value ?? $event.target.value"></nldd-text-field>
          </nldd-form-field>
          <nldd-form-field :label="selfDeclared ? 'Toelichting' : 'Waarom klopt het geregistreerde gegeven niet?'" :optional="selfDeclared || undefined">
            <nldd-multi-line-text-field :value="reason" rows="3" placeholder="Bijvoorbeeld: mijn inkomen is dit jaar lager door minder opdrachten." @input="reason = $event.detail?.value ?? $event.target.value"></nldd-multi-line-text-field>
            <nldd-form-field-help-text>{{ selfDeclared || profile?.feature_flags?.AUTO_APPROVE_CLAIMS ? 'Uw opgave wordt direct gebruikt in de berekening.' : 'Een behandelaar beoordeelt de correctie; tot die tijd rekent de wet met het geregistreerde gegeven.' }}</nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-banner v-if="error" variant="critical" :text="error"></nldd-banner>
          <nldd-form-actions>
            <nldd-button-group orientation="horizontal">
              <nldd-button variant="primary" :text="selfDeclared ? 'Opgeven' : 'Correctie indienen'" @click="submit"></nldd-button>
              <nldd-button variant="secondary" text="Annuleren" @click="emit('close')"></nldd-button>
            </nldd-button-group>
          </nldd-form-actions>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
