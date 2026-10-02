<script setup>
// Log in through a channel of the process (declared in the actor's policy,
// its technique in channels.yaml of the deployment, RFC-047): the label, the explanation and the fields come from the
// channel. Every channel is simulated: the process only checks the form of
// what you enter, not who you are. When more than one role logs in through
// the channel, the chosen role is sent along. When the process provides
// login examples for this channel, there is a button per example below to
// log in with it directly or to fill in the form with it.
import { computed, inject, ref } from 'vue';
import { fieldText } from '../form.js';
import { emptyFields } from '../channel.js';

const props = defineProps({
  // The id of the channel and its description from GET /api/processes.
  channelId: { type: String, required: true },
  channel: { type: Object, required: true },
  // The role to log in as; sent along when the channel has more than one.
  role: { type: String, default: null },
  sendRole: { type: Boolean, default: false },
});

const api = inject('api');
const examples = inject('examples');

const emit = defineEmits(['logged-in']);

const values = ref(emptyFields(props.channel));
const error = ref('');
const busy = ref(false);

// The examples of this channel, with their fields in the order of the channel.
const own = computed(() => examples.value.logins.filter((e) => e.channel === props.channelId));

function description(example) {
  return props.channel.fields.map((f) => example.fields[f.name]).filter(Boolean).join(', ');
}

function fillIn(example) {
  values.value = emptyFields(props.channel, example.fields);
}

async function login() {
  if (busy.value) return;
  error.value = '';
  busy.value = true;
  try {
    const input = { ...values.value };
    if (props.sendRole && props.role) input.role = props.role;
    emit('logged-in', await api.login(props.channelId, input));
  } catch (e) {
    error.value = e.message;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <nldd-title size="2"><h1>{{ channel.label }}</h1></nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>Dit is een nagebootste inlog: het proces controleert alleen de vorm van wat u invult, niet wie u bent.</p>
    <p v-if="channel.explanation">{{ channel.explanation }}</p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <nldd-form novalidate @submit.prevent="login">
    <nldd-form-field v-for="f in channel.fields" :key="f.name" :label="f.label">
      <nldd-text-field
        :value="values[f.name]"
        :name="f.name"
        :keyboard="f.numeric ? 'numeric' : undefined"
        @input="values = { ...values, [f.name]: fieldText($event) }"
      ></nldd-text-field>
    </nldd-form-field>
    <template v-if="error">
      <nldd-inline-dialog variant="alert" text="Inloggen lukt niet" :supporting-text="error"></nldd-inline-dialog>
    </template>
    <nldd-form-actions>
      <nldd-button variant="primary" type="submit" text="Inloggen" :loading="busy || undefined"></nldd-button>
    </nldd-form-actions>
  </nldd-form>
  <template v-if="own.length">
    <nldd-spacer size="32"></nldd-spacer>
    <nldd-title size="4"><h2>Voorbeelden</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-table columns="auto minmax(240px,1fr)" accessible-label="Inlogvoorbeelden">
      <nldd-table-row v-for="e in own" :key="e.label">
        <nldd-cell>
          <nldd-button-group orientation="horizontal">
            <nldd-button variant="secondary" size="sm" text="Vul in" :disabled="busy || undefined" :accessible-label="`Vul in met ${e.label}`" @click="fillIn(e)"></nldd-button>
            <nldd-button
              variant="secondary"
              size="sm"
              text="Inloggen"
              :accessible-label="`Inloggen met ${e.label}`"
              :loading="busy || undefined"
              :disabled="busy || undefined"
              @click="fillIn(e); login()"
            ></nldd-button>
          </nldd-button-group>
        </nldd-cell>
        <nldd-text-cell :text="e.label" :supporting-text="description(e)"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </template>
</template>
