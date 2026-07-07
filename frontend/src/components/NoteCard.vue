<script setup>
/**
 * NoteCard — the inner rendering of a single note: comment (rich-text), author
 * byline, and a per-note action toolbar (edit / share / delete). Shared by the
 * Notities pane and the annotation-badge popover so both render identically.
 *
 * Emits edit/share/delete; the parent owns the actual actions (and their confirm
 * modals) and the group/span context an edit needs. The card renders only the
 * inner content — the caller wraps it in a card/container as needed.
 */
import { computed } from 'vue';

const props = defineProps({
  note: { type: Object, required: true },
  // Whether the signed-in user has traject write access (needed to share).
  canEdit: { type: Boolean, default: false },
  // A share is in flight — disables the share button.
  saving: { type: Boolean, default: false },
});
defineEmits(['edit', 'share', 'delete']);

// The note's primary body value: the comment, or a linked source.
const detailText = computed(() => {
  const body = Array.isArray(props.note?.body) ? props.note.body[0] : props.note?.body;
  return body?.value || body?.source || '';
});
// Author display name, tolerating the { id, name } shape and legacy strings.
const creatorName = computed(() => props.note?.creator?.name ?? props.note?.creator ?? '');
// A draft lives in this browser's localStorage, so it is the user's by
// construction: deletable, and shareable when a traject is active. Committed
// notes are not __draft, so they carry no actions here.
const isDraft = computed(() => !!props.note?.__draft);
const canDelete = computed(() => isDraft.value);
const canShare = computed(() => isDraft.value && props.canEdit);
</script>

<template>
  <nldd-rich-text>
    <p>{{ detailText || '—' }}</p>
  </nldd-rich-text>
  <template v-if="creatorName">
    <nldd-spacer size="4"></nldd-spacer>
    <nldd-byline :text="creatorName"></nldd-byline>
  </template>
  <template v-if="canShare || canDelete">
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-toolbar size="md" label="Notitie-acties">
      <nldd-toolbar-item v-if="canDelete" slot="start" label="Bewerken">
        <nldd-icon-button
          icon="edit"
          text="Bewerken"
          variant="secondary"
          size="md"
          @click="$emit('edit', $event)"
        ></nldd-icon-button>
        <nldd-menu-item slot="overflow" icon="edit" text="Bewerken" @select="$emit('edit', $event)"></nldd-menu-item>
      </nldd-toolbar-item>
      <nldd-toolbar-item v-if="canShare" slot="start" label="Delen">
        <nldd-button
          text="Delen"
          variant="secondary"
          size="md"
          :disabled="saving || undefined"
          @click="$emit('share')"
        ></nldd-button>
        <nldd-menu-item slot="overflow" icon="share" text="Delen" @select="$emit('share')"></nldd-menu-item>
      </nldd-toolbar-item>
      <nldd-toolbar-item v-if="canDelete" slot="end" label="Verwijderen">
        <nldd-icon-button
          icon="trash"
          text="Verwijderen"
          variant="destructive"
          size="md"
          @click="$emit('delete')"
        ></nldd-icon-button>
        <nldd-menu-item slot="overflow" icon="trash" text="Verwijderen" @select="$emit('delete')"></nldd-menu-item>
      </nldd-toolbar-item>
    </nldd-toolbar>
  </template>
</template>
