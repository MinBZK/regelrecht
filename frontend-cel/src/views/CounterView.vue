<script setup>
// The counter: an application that came in some other way (on paper, at the
// desk), entered by an employee on behalf of the applicant. The counter
// identifies the applicant with the fields of a portal channel (from the
// configuration of the process); nobody logged in with it. The day of receipt
// counts in law (Awb 4:13: the decision period runs from receipt) and becomes
// the effective_at of the gram; the entry is recorded_at. The process refuses
// a day after today, and a day before the opening of the window when the
// policy names one.
import { computed, inject, ref } from 'vue';
import { fieldText } from '../form.js';
import { emptyFields, portalChannels } from '../channel.js';
import ApplicationView from './ApplicationView.vue';
import InputField from '../components/InputField.vue';

const api = inject('api');
const process = inject('process');
const emit = defineEmits(['submitted']);

const channels = computed(() => portalChannels(process));
const channelId = ref(channels.value[0]?.id ?? null);
const channel = computed(() => channels.value.find((c) => c.id === channelId.value) ?? null);
const applicant = ref(emptyFields(channel.value));
const receivedAt = ref(null);

function chooseChannel(id) {
  channelId.value = id;
  applicant.value = emptyFields(channel.value);
}

function send(external) {
  return api.submitAtCounter({
    applicant: { channel: channelId.value, ...applicant.value },
    received_at: receivedAt.value ?? '',
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
  <ApplicationView :send="send" :with-assessment="false" title="Aanvraag invoeren aan het loket" @submitted="emit('submitted', $event)">
    <template #before>
      <nldd-form-section text="Aanvrager en ontvangst"></nldd-form-section>
      <nldd-form-field v-if="channels.length > 1" label="Aanvrager aangeduid met">
        <nldd-segmented-control accessible-label="Kanaal van de aanvrager" :value="channelId" @change="chooseChannel($event.detail.value)">
          <nldd-segmented-control-item v-for="c in channels" :key="c.id" :value="c.id" :text="c.role"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-form-field>
      <nldd-form-field v-for="f in channel?.fields ?? []" :key="channelId + f.name" :label="f.label">
        <nldd-text-field
          :value="applicant[f.name]"
          :name="`applicant-${f.name}`"
          :keyboard="f.numeric ? 'numeric' : undefined"
          @input="applicant = { ...applicant, [f.name]: fieldText($event) }"
        ></nldd-text-field>
      </nldd-form-field>
      <nldd-form-field label="Ontvangen op" supporting-label="de datumstempel">
        <InputField kind="date" label="Ontvangen op" :model-value="receivedAt" @update:model-value="receivedAt = $event" />
      </nldd-form-field>
    </template>
  </ApplicationView>
</template>
