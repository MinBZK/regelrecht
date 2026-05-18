<script setup>
import { computed, ref, useTemplateRef } from 'vue';
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

function bodies(note) {
  return Array.isArray(note?.body) ? note.body : note?.body ? [note.body] : [];
}
function noteText(note) {
  // W3C allows a plain-string body shorthand (`body: "text"`) alongside the
  // structured TextualBody form; surface either, but not the tagging body
  // (that is shown as chips).
  return (
    bodies(note).find((b) => typeof b === 'string') ??
    bodies(note).find((b) => b?.type === 'TextualBody' && b.purpose !== 'tagging')?.value ??
    ''
  );
}
function noteLink(note) {
  return bodies(note).find((b) => b?.type === 'SpecificResource')?.source || '';
}
function noteTags(note) {
  return bodies(note)
    .filter((b) => b?.type === 'TextualBody' && b.purpose === 'tagging')
    .map((b) => b.value);
}
function noteCreator(note) {
  if (!note?.creator) return '';
  return typeof note.creator === 'string' ? note.creator : note.creator.name || '';
}

// --- Hover popover -------------------------------------------------------
// One shared nldd-popover. On hover/focus of a highlight we point its
// anchorElement at that mark, set the active note, and open it. nldd-popover
// is a native HTML popover (click/light-dismiss by design), so hover is wired
// manually with a small close delay so the pointer can travel mark -> popover
// without it snapping shut.
const popoverEl = useTemplateRef('popoverEl');
const activeNote = ref(null);
let closeTimer = null;

function openFor(event, note) {
  if (closeTimer) {
    clearTimeout(closeTimer);
    closeTimer = null;
  }
  activeNote.value = note;
  const pop = popoverEl.value;
  if (!pop) return;
  pop.anchorElement = event.currentTarget;
  // showPopover throws if already open; guard with matches(':popover-open').
  try {
    if (!pop.matches?.(':popover-open')) pop.showPopover?.();
  } catch {
    /* popover already open against another anchor — anchorElement moved it */
  }
}

function scheduleClose() {
  if (closeTimer) clearTimeout(closeTimer);
  closeTimer = setTimeout(() => {
    popoverEl.value?.hidePopover?.();
    activeNote.value = null;
    closeTimer = null;
  }, 160);
}

function cancelClose() {
  if (closeTimer) {
    clearTimeout(closeTimer);
    closeTimer = null;
  }
}
</script>

<template>
  <template v-if="article">
    <nldd-rich-text>
      <p>
        <template v-for="(seg, i) in segments" :key="i">
          <mark
            v-if="seg.note"
            :class="[
              motivationClass(seg.note),
              authorityClass(seg.note),
              { 'is-active': activeNote === seg.note },
            ]"
            tabindex="0"
            :aria-label="`Notitie: ${seg.note.motivation}`"
            @mouseenter="openFor($event, seg.note)"
            @mouseleave="scheduleClose"
            @focus="openFor($event, seg.note)"
            @blur="scheduleClose"
            >{{ seg.text }}</mark
          >
          <template v-else>{{ seg.text }}</template>
        </template>
      </p>
    </nldd-rich-text>

    <!-- Single shared popover; anchorElement is repointed per hovered mark. -->
    <nldd-popover
      ref="popoverEl"
      accessible-label="Notitie"
      placement="bottom-start"
      @mouseenter="cancelClose"
      @mouseleave="scheduleClose"
    >
      <div v-if="activeNote" class="note-pop" :class="motivationClass(activeNote)">
        <div class="note-pop__head">
          <span class="note-pop__badge">{{ activeNote.motivation }}</span>
          <span v-if="noteCreator(activeNote)" class="note-pop__creator">{{
            noteCreator(activeNote)
          }}</span>
        </div>
        <p v-if="noteText(activeNote)" class="note-pop__body">{{ noteText(activeNote) }}</p>
        <a
          v-if="noteLink(activeNote)"
          class="note-pop__link"
          :href="noteLink(activeNote)"
          @click.prevent
          >{{ noteLink(activeNote) }}</a
        >
        <div v-if="noteTags(activeNote).length" class="note-pop__tags">
          <span v-for="t in noteTags(activeNote)" :key="t" class="note-pop__tag">{{ t }}</span>
        </div>
        <span
          v-if="activeNote.workflow"
          class="note-pop__workflow"
          :data-state="activeNote.workflow"
          >{{ activeNote.workflow === 'open' ? 'open vraag' : 'afgehandeld' }}</span
        >
      </div>
    </nldd-popover>
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
  cursor: help;
  transition: filter 0.12s;
  /* Authority -> border style. */
}
mark.is-active {
  filter: brightness(1.15) saturate(1.3);
}
mark.note-authoritative {
  border-bottom: 2px solid currentColor;
}
mark.note-generated {
  border-bottom: 2px dotted currentColor;
}
/* Reserved: authorityClass() returns 'note-advisory' once the
   competent_authority wiring lands (RFC-018 Decision 3). Intentionally
   kept ahead of its producer — not dead code. */
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

/* Popover card content. Border-left echoes the highlight colour so the
   popover is visually tied to the mark it describes. */
.note-pop {
  border-left: 3px solid transparent;
  padding-left: 10px;
}
.note-pop.note-linking {
  border-left-color: #3b82f6;
}
.note-pop.note-commenting {
  border-left-color: #eab308;
}
.note-pop.note-questioning {
  border-left-color: #f97316;
}
.note-pop.note-tagging {
  border-left-color: #22c55e;
}
.note-pop.note-other {
  border-left-color: #94a3b8;
}
.note-pop__head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.note-pop__badge {
  font-size: 0.72rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  opacity: 0.7;
}
.note-pop__creator {
  font-size: 0.72rem;
  opacity: 0.55;
  margin-left: auto;
}
.note-pop__body {
  margin: 0;
  font-size: 0.88rem;
  line-height: 1.5;
}
.note-pop__link {
  display: inline-block;
  margin-top: 8px;
  font-size: 0.8rem;
  word-break: break-all;
}
.note-pop__tags {
  margin-top: 10px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.note-pop__tag {
  font-size: 0.72rem;
  padding: 1px 8px;
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.2);
}
.note-pop__workflow {
  display: inline-block;
  margin-top: 10px;
  font-size: 0.72rem;
  padding: 1px 8px;
  border-radius: 4px;
}
.note-pop__workflow[data-state='open'] {
  background: rgba(249, 115, 22, 0.18);
  color: #c2410c;
}
.note-pop__workflow[data-state='resolved'] {
  background: rgba(34, 197, 94, 0.18);
  color: #15803d;
}
</style>
