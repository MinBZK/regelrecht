<script setup>
import { computed } from 'vue';
import { markRanges } from '../composables/useNotes.js';

const props = defineProps({
  article: { type: Object, default: null },
  // [{ note, spans }] for the current article, from useNotes().notesForArticle
  notesForArticle: { type: Array, default: () => [] },
});

// The resolver matched against the raw article text, so offsets are into that
// exact string. Rendering markdown here would desync the offsets, so notes are
// shown over plain text. (ArticleText.vue keeps the markdown view for the
// non-annotated Tekst pane.)
const segments = computed(() => {
  if (!props.article?.text) return [];
  return markRanges(props.article.text, props.notesForArticle);
});

// W3C motivation -> colour class. Linking blue, commenting yellow,
// questioning orange, tagging green (RFC-018 Decision 10).
function motivationClass(note) {
  const m = note?.motivation;
  if (m === 'linking') return 'note-linking';
  if (m === 'commenting') return 'note-commenting';
  if (m === 'questioning') return 'note-questioning';
  if (m === 'tagging') return 'note-tagging';
  return 'note-other';
}

// Authority is derived from the creator (RFC-018 Decision 3). The editor does
// not yet know each law's competent_authority, so for now: a known tool
// creator => generated (dotted), anything else => default (solid). Advisory
// (dashed) is reserved for when competent_authority wiring lands.
function authorityClass(note) {
  const c = (note?.creator || '').toString().toLowerCase();
  if (c.includes('tool') || c.includes('llm') || c.includes('generated')) {
    return 'note-generated';
  }
  return 'note-authoritative';
}

function noteTitle(note) {
  const body = Array.isArray(note?.body) ? note.body : note?.body ? [note.body] : [];
  const text = body.find((b) => b?.type === 'TextualBody')?.value;
  const link = body.find((b) => b?.type === 'SpecificResource')?.source;
  return `${note?.motivation ?? 'note'}${text ? `: ${text}` : link ? ` → ${link}` : ''}`;
}
</script>

<template>
  <template v-if="article">
    <nldd-rich-text>
      <!-- Phase 4 is display-only. The marks are not interactive yet, so no
           role="button"/tabindex (that would announce an action that does
           nothing — an accessibility lie). The note content is surfaced via
           the title tooltip. Phase 5 adds a detail panel and makes the marks
           focusable + keyboard-activatable. -->
      <p>
        <template v-for="(seg, i) in segments" :key="i">
          <mark
            v-if="seg.note"
            :class="[motivationClass(seg.note), authorityClass(seg.note)]"
            :title="noteTitle(seg.note)"
            >{{ seg.text }}</mark
          >
          <template v-else>{{ seg.text }}</template>
        </template>
      </p>
    </nldd-rich-text>
  </template>
  <nldd-inline-dialog v-else text="Geen artikel geselecteerd"></nldd-inline-dialog>
</template>

<style scoped>
/* Legal text carries its own paragraph breaks (\n\n). Preserve them so the
   Notities pane reads like the other text panes instead of one collapsed
   block. pre-wrap keeps wrapping while honouring newlines. */
p {
  white-space: pre-wrap;
}
mark {
  padding: 0 0.1em;
  border-radius: 2px;
  /* Authority -> border style. */
}
mark.note-authoritative {
  border-bottom: 2px solid currentColor;
}
mark.note-generated {
  border-bottom: 2px dotted currentColor;
}
mark.note-advisory {
  border-bottom: 2px dashed currentColor;
}
/* Motivation -> background colour. Kept light so text stays readable in
   both themes; the design system's tokens would be preferable once the
   notes feature has dedicated tokens. */
mark.note-linking {
  background: rgba(59, 130, 246, 0.28);
}
mark.note-commenting {
  background: rgba(234, 179, 8, 0.28);
}
mark.note-questioning {
  background: rgba(249, 115, 22, 0.3);
}
mark.note-tagging {
  background: rgba(34, 197, 94, 0.28);
}
mark.note-other {
  background: rgba(148, 163, 184, 0.28);
}
mark:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 1px;
}
</style>
