<script setup>
import { useId } from 'vue';

/**
 * Compact row-actions control: a single "more" icon-button that opens a
 * menu with Edit + Delete. Replaces the inline "Bewerk" button + "—"
 * delete icon-button pair to save horizontal space in dense list rows
 * (machine-readable definitions / parameters / inputs / outputs /
 * actions).
 *
 * Emits `edit` and `delete`; the parent owns the actual handlers and the
 * delete-confirmation flow. The test-id props are forwarded onto the
 * matching menu-items so existing data-testid selectors keep working.
 */
defineProps({
  // Accessible label / tooltip for the trigger (e.g. "Acties voor input bsn").
  accessibleLabel: { type: String, default: 'Acties' },
  editTestid: { type: String, default: undefined },
  deleteTestid: { type: String, default: undefined },
});

defineEmits(['edit', 'delete']);

// Unique id so multiple instances in the same list anchor their own menu.
const anchorId = `row-actions-${useId()}`;
</script>

<template>
  <nldd-icon-button
    :id="anchorId"
    icon="more"
    :text="accessibleLabel"
    tooltip-timing="never"
    variant="neutral-tinted"
  ></nldd-icon-button>
  <nldd-menu :anchor="anchorId">
    <nldd-menu-item
      text="Bewerk"
      icon="edit"
      :data-testid="editTestid"
      @click.stop="$emit('edit')"
    ></nldd-menu-item>
    <nldd-menu-divider></nldd-menu-divider>
    <nldd-menu-item
      text="Verwijder"
      icon="delete"
      :data-testid="deleteTestid"
      @click.stop="$emit('delete')"
    ></nldd-menu-item>
  </nldd-menu>
</template>
