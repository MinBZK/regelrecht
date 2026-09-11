<script setup>
import { computed } from 'vue';
import ActionCard from './ActionCard.vue';
import { actionsByActor } from '../world/snapshot.js';

// Wat er nu te doen is, gegroepeerd per actor: elke cel die iets kan doen krijgt
// haar eigen kopje met de acties die bij haar horen.

const props = defineProps({
  /** Het beeld van de wereld. */
  snapshot: { type: Object, default: null },
  /** Staat er een wijziging onderweg? */
  busy: { type: Boolean, default: false },
});

const emit = defineEmits(['run']);

const groups = computed(() => actionsByActor(props.snapshot));
</script>

<template>
  <nldd-container layout="stack" gap="24">
    <nldd-container v-for="group in groups" :key="group.actor" layout="stack" gap="12">
      <nldd-title size="5">
        <span slot="overline">Actor</span>
        <span>{{ group.actor }}</span>
        <span slot="subtitle">{{ group.actions.length }} {{ group.actions.length === 1 ? 'actie' : 'acties' }}</span>
      </nldd-title>
      <ActionCard
        v-for="action in group.actions"
        :key="action.id"
        :action="action"
        :busy="busy"
        @run="emit('run', $event)"
      />
    </nldd-container>

    <nldd-inline-dialog
      v-if="groups.length === 0"
      icon="hand"
      text="Geen acties"
      supporting-text="Dit wereldbestand beschrijft geen acties."
    ></nldd-inline-dialog>
  </nldd-container>
</template>
