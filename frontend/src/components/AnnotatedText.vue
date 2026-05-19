<script setup>
import { computed, ref, watch, nextTick, useTemplateRef, onBeforeUnmount } from 'vue';
import { renderArticleHtml } from '../composables/useArticleMarkdown.js';
import {
  buildAlignment,
  spanToNodeSlices,
  cpToUtf16,
} from '../composables/useNotesHighlight.js';
import { selectionToRawRange } from '../composables/useTextSelection.js';
import NoteCreator from './NoteCreator.vue';

const props = defineProps({
  article: { type: Object, default: null },
  // [{ note, spans }] for the current article, from useNotes().notesForArticle
  notesForArticle: { type: Array, default: () => [] },
  // Note authoring (RFC-018 write path). Off keeps the pure read-only view.
  canCreate: { type: Boolean, default: false },
  lawId: { type: String, default: '' },
  // Loaded WASM engine, for selector uniqueness validation in NoteCreator.
  engine: { type: Object, default: null },
});

const emit = defineEmits(['create-note']);

// Render the law text as markdown, identical to the Tekst pane with notes
// off (shared pipeline so the two cannot drift — #646). The resolver matched
// char-offsets into the *raw* text; after rendering we re-anchor those onto
// the DOM's text nodes and wrap each resolved span in a <mark> imperatively,
// because the spans cross marked's generated <li>/<p> structure and cannot be
// expressed as a flat string partition (that was the old markRanges path,
// kept in useNotes.js as the reference implementation + its tests).
const html = computed(() => renderArticleHtml(props.article?.text || ''));

const richTextEl = useTemplateRef('richTextEl');

// Index notes so a <mark> can carry data-note-idx and the delegated hover
// handler can recover the note object without per-mark Vue bindings.
const noteByIdx = computed(() => props.notesForArticle.map((n) => n.note));

function collectTextNodes(root) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const out = [];
  let n;
  while ((n = walker.nextNode())) {
    out.push({ node: n, text: n.textContent });
  }
  return out;
}

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

// Authority -> border style. Derived from the creator (RFC-018 Decision 3):
// a known tool creator => generated (dotted), anything else => default
// (solid). Advisory (dashed) is reserved for when competent_authority wiring
// lands.
function authorityClass(note) {
  const c = (note?.creator || '').toString().toLowerCase();
  if (c.includes('tool') || c.includes('llm') || c.includes('generated')) {
    return 'note-generated';
  }
  return 'note-authoritative';
}

// Wrap one code-point slice of a text node in a <mark>. Splits the text node
// so the Range covers exactly the slice, then surrounds it. Returns the
// created <mark> (or null if the slice is degenerate after splitting).
function wrapSlice(slice, noteIdx, note) {
  const textNode = slice.node;
  const full = textNode.textContent;
  const u1 = cpToUtf16(full, slice.startCp);
  const u2 = cpToUtf16(full, slice.endCp);
  if (u2 <= u1) return null;
  const range = document.createRange();
  range.setStart(textNode, u1);
  range.setEnd(textNode, u2);
  const mark = document.createElement('mark');
  mark.dataset.noteIdx = String(noteIdx);
  // Class instead of data attribute so the scoped :deep() rules below match;
  // mirrors the colour/border scheme of the pre-#646 string renderer.
  mark.className = `${motivationClass(note)} ${authorityClass(note)}`;
  mark.setAttribute('aria-label', `Notitie: ${note?.motivation ?? ''}`);
  mark.setAttribute('tabindex', '0');
  try {
    range.surroundContents(mark);
  } catch {
    // surroundContents throws if the range partially selects a non-text
    // node; our ranges are always within a single text node, so this is
    // defensive only.
    range.detach?.();
    return null;
  }
  return mark;
}

// Unwrap every <mark> we added, restoring the original text nodes. v-html
// resets the DOM when `html` changes, but a notes-only change (article
// unchanged — e.g. once notes become editable) leaves the prior marks in
// place, so applyHighlights must be idempotent and clean up first.
function clearHighlights(root) {
  for (const mark of root.querySelectorAll('mark[data-note-idx]')) {
    const parent = mark.parentNode;
    if (!parent) continue;
    while (mark.firstChild) parent.insertBefore(mark.firstChild, mark);
    parent.removeChild(mark);
    parent.normalize(); // re-join the split text nodes so offsets are stable
  }
}

// Vue paints the sanitized markdown via v-html, then we layer the marks on
// top. Runs after every relevant change (article or notes). Idempotent:
// clears prior marks, re-aligns, re-wraps.
function applyHighlights() {
  const root = richTextEl.value;
  if (!root) return;
  clearHighlights(root);

  const rawText = props.article?.text || '';
  const notes = props.notesForArticle;
  if (!rawText || notes.length === 0) return;

  const domNodes = collectTextNodes(root);
  if (domNodes.length === 0) return;

  const { rawToDom, desynced } = buildAlignment(rawText, domNodes);
  // If the rendered text diverged from the raw text by more than the known
  // raw-only transforms (list prefixes, collapsed whitespace) the offset map
  // is untrustworthy. Better to show the clean markdown with no highlights
  // than to smear every mark onto the wrong words. Surfaced via useNotes'
  // `issues` is out of scope here; the spans simply do not render.
  if (desynced) return;
  const domNodeCpLen = new Map(
    domNodes.map(({ node, text }) => [node, Array.from(text).length]),
  );

  // Flatten to (span, note) and wrap by descending raw start. surroundContents
  // splits the text node, so processing the rightmost span first keeps every
  // earlier span's node references and offsets valid. `wrapped` tracks the raw
  // ranges already marked: a span that overlaps one is dropped, matching the
  // deterministic drop in markRanges() (RFC-018's overlapping-notes case).
  const flat = [];
  notes.forEach((entry, idx) => {
    for (const span of entry.spans) flat.push({ span, idx });
  });
  // Drop decision must use document order (ascending), so the *earlier* note
  // wins the overlap, same as markRanges. Wrapping order stays descending.
  const ascending = [...flat].sort(
    (a, b) => a.span.start - b.span.start || b.span.end - a.span.end,
  );
  const keep = new Set();
  let cursor = 0;
  for (const item of ascending) {
    if (item.span.start < cursor) continue; // overlaps an earlier kept span
    keep.add(item);
    cursor = item.span.end;
  }

  const descending = flat
    .filter((item) => keep.has(item))
    .sort((a, b) => b.span.start - a.span.start);

  for (const { span, idx } of descending) {
    const note = noteByIdx.value[idx];
    const slices = spanToNodeSlices(rawToDom, span, domNodeCpLen);
    // Wrap right-to-left within the span too, same node-stability reason.
    for (let i = slices.length - 1; i >= 0; i--) {
      wrapSlice(slices[i], idx, note);
    }
  }
}

// deep so an in-place mutation of notesForArticle (push/splice from the
// upcoming editable-notes path) re-applies, not only a reference swap. The
// await nextTick() ensures Vue has patched v-html before applyHighlights
// walks the DOM (it also clears prior marks first, so a stale callback from
// a rapid change is self-healing rather than additive).
watch(
  [html, () => props.notesForArticle],
  async () => {
    await nextTick();
    applyHighlights();
  },
  { immediate: true, deep: true },
);

// --- Hover popover -------------------------------------------------------
// One shared nldd-popover. Marks are created imperatively, so hover is wired
// via event delegation on the container instead of per-<mark> @mouseenter.
// nldd-popover is a native HTML popover (click/light-dismiss by design), so
// hover is wired manually with a small close delay so the pointer can travel
// mark -> popover without it snapping shut.
const popoverEl = useTemplateRef('popoverEl');
const activeNote = ref(null);
let closeTimer = null;

function markFromEvent(event) {
  const el = event.target?.closest?.('mark[data-note-idx]');
  if (!el) return null;
  const idx = Number(el.dataset.noteIdx);
  return { el, note: noteByIdx.value[idx] || null };
}

function openFor(el, note) {
  if (closeTimer) {
    clearTimeout(closeTimer);
    closeTimer = null;
  }
  activeNote.value = note;
  const pop = popoverEl.value;
  if (!pop) return;
  pop.anchorElement = el;
  try {
    if (!pop.matches?.(':popover-open')) pop.showPopover?.();
  } catch {
    /* already open against another anchor — anchorElement moved it */
  }
}

function onPointerOver(event) {
  const hit = markFromEvent(event);
  if (hit?.note) openFor(hit.el, hit.note);
}
function onPointerOut(event) {
  if (markFromEvent(event)) scheduleClose();
}
function onFocusIn(event) {
  const hit = markFromEvent(event);
  if (hit?.note) openFor(hit.el, hit.note);
}
function onFocusOut(event) {
  if (markFromEvent(event)) scheduleClose();
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
onBeforeUnmount(() => {
  if (closeTimer) clearTimeout(closeTimer);
});

function bodies(note) {
  return Array.isArray(note?.body) ? note.body : note?.body ? [note.body] : [];
}
function noteText(note) {
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

// --- Note authoring (RFC-018 write path) --------------------------------
// On a non-empty selection inside the rendered text, show a small "Notitie"
// button at the selection. Clicking it maps the selection to a raw char range
// (selectionToRawRange handles the markdown DOM -> raw text gap) and opens
// NoteCreator. The button is positioned over the selection's bounding rect.
const selectionRange = ref(null); // raw [start,end) for NoteCreator
const creatorOpen = ref(false);
const selBtnStyle = ref(null); // position of the floating "Notitie" button
const anchorStyle = ref(null); // position of the popover anchor (persists)
const selAnchorEl = useTemplateRef('selAnchorEl');

// The raw [start,end) is captured here, at selectionchange time, NOT at
// button-click time. applyHighlights mutates the DOM (it wraps <mark> and
// calls parent.normalize(), which destroys the live Selection's boundary
// nodes); if the DOM->raw mapping were deferred to openCreator() a draft
// resolving between selection and click would collapse the selection and the
// note could not be created while any other note is highlighted. Mapping the
// selection the instant it is made — while the live selection and the DOM it
// was made against are still in sync — removes that race entirely.
const pendingRange = ref(null);

function clearSelectionUi() {
  if (creatorOpen.value) return; // keep anchor + form while the form is open
  selBtnStyle.value = null;
  anchorStyle.value = null;
  selectionRange.value = null;
  pendingRange.value = null;
}

function onSelectionChange() {
  if (!props.canCreate || creatorOpen.value) return;
  const root = richTextEl.value;
  const sel = window.getSelection?.();
  if (!root || !sel || sel.rangeCount === 0 || sel.isCollapsed) {
    clearSelectionUi();
    return;
  }
  const domRange = sel.getRangeAt(0);
  if (!root.contains(domRange.commonAncestorContainer)) {
    clearSelectionUi();
    return;
  }
  const rect = domRange.getBoundingClientRect();
  const wrap = root.closest('.annotated-wrap');
  const wrapRect = wrap?.getBoundingClientRect();
  if (!wrapRect || rect.width === 0) {
    clearSelectionUi();
    return;
  }
  // Map the selection to raw offsets NOW, against the DOM it was made in.
  const rawText = props.article?.text || '';
  const range = rawText ? selectionToRawRange(rawText, root) : null;
  if (!range) {
    // Unmappable selection (spans only stripped structure, or the render
    // desynced) — no actionable note, drop the UI.
    clearSelectionUi();
    return;
  }
  pendingRange.value = range;
  // Position the button just below the selection, relative to the wrap. The
  // anchor sits at the selection start so the popover opens against the text.
  const top = rect.bottom - wrapRect.top + 6;
  const left = rect.left - wrapRect.left;
  selBtnStyle.value = { position: 'absolute', top: `${top}px`, left: `${left}px`, zIndex: 5 };
  anchorStyle.value = {
    position: 'absolute',
    top: `${rect.top - wrapRect.top}px`,
    left: `${left}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
    pointerEvents: 'none',
  };
}

function openCreator() {
  if (!pendingRange.value) return;
  selectionRange.value = pendingRange.value;
  creatorOpen.value = true;
  selBtnStyle.value = null; // hide the button; the anchor persists for the popover
}

function resetCreator() {
  creatorOpen.value = false;
  selectionRange.value = null;
  pendingRange.value = null;
  anchorStyle.value = null;
}

function onNoteCreated(note) {
  resetCreator();
  window.getSelection?.()?.removeAllRanges();
  emit('create-note', note);
}

function onCreatorCancel() {
  resetCreator();
}

// The selected article can change while the creator is open (router nav).
// selectionRange holds offsets into the OLD article text; NoteCreator's
// :raw-text would switch to the new article and buildSelector would slice a
// garbage substring at stale offsets — potentially persisting a note on the
// wrong text. Tear the creation flow down on any article change.
watch(
  () => props.article,
  () => {
    resetCreator();
    selBtnStyle.value = null;
  },
);

// selectionchange fires on every caret move during a drag. onSelectionChange
// rebuilds the raw<->DOM alignment (a full text-node walk), so debounce to
// the selection settling rather than running it per tick.
let selChangeTimer = null;
function onSelectionChangeDebounced() {
  if (selChangeTimer) clearTimeout(selChangeTimer);
  selChangeTimer = setTimeout(() => {
    selChangeTimer = null;
    onSelectionChange();
  }, 120);
}

watch(
  () => props.canCreate,
  (on) => {
    if (on) {
      document.addEventListener('selectionchange', onSelectionChangeDebounced);
    } else {
      document.removeEventListener(
        'selectionchange',
        onSelectionChangeDebounced,
      );
      if (selChangeTimer) {
        clearTimeout(selChangeTimer);
        selChangeTimer = null;
      }
      clearSelectionUi();
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  document.removeEventListener('selectionchange', onSelectionChangeDebounced);
  if (selChangeTimer) clearTimeout(selChangeTimer);
});
</script>

<template>
  <template v-if="article">
    <!-- Delegated hover/focus: marks are added imperatively after render. -->
    <div
      class="annotated-wrap"
      @pointerover="onPointerOver"
      @pointerout="onPointerOut"
      @focusin="onFocusIn"
      @focusout="onFocusOut"
    >
      <nldd-rich-text ref="richTextEl" v-html="html"></nldd-rich-text>

      <!-- Note authoring (RFC-018). The anchor span tracks the selection
           rect and persists while the form is open so NoteCreator's popover
           stays attached to the text after the native selection is gone. -->
      <span
        v-if="canCreate && anchorStyle"
        ref="selAnchorEl"
        class="sel-anchor"
        :style="anchorStyle"
      ></span>
      <span v-if="canCreate && selBtnStyle" class="sel-btn" :style="selBtnStyle">
        <nldd-button
          size="sm"
          variant="primary"
          text="Notitie"
          data-testid="create-note-btn"
          @click="openCreator"
        ></nldd-button>
      </span>

      <NoteCreator
        v-if="canCreate"
        :range="creatorOpen ? selectionRange : null"
        :raw-text="article?.text || ''"
        :law-id="lawId"
        :article="article"
        :engine="engine"
        :anchor="selAnchorEl"
        @create="onNoteCreated"
        @cancel="onCreatorCancel"
      />
    </div>

    <!-- Single shared popover; anchorElement is repointed per hovered mark. -->
    <nldd-popover
      ref="popoverEl"
      accessible-label="Notitie"
      placement="bottom-start"
      width="380px"
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
/* Positioning context for the selection button + popover anchor, which are
   placed absolutely over the selection rect (RFC-018 write path). */
.annotated-wrap {
  position: relative;
}
.sel-anchor {
  display: block;
}

/* Marks are wrapped imperatively into nldd-rich-text's slotted light DOM, so
   they are not tagged with Vue's scoped data attribute; reach them with
   :deep(). Motivation -> background colour, authority -> border style, same
   scheme as the pre-#646 string renderer. */
.annotated-wrap :deep(mark) {
  padding: 0 0.1em;
  border-radius: 2px;
  cursor: help;
}
.annotated-wrap :deep(mark.note-authoritative) {
  border-bottom: 2px solid currentColor;
}
.annotated-wrap :deep(mark.note-generated) {
  border-bottom: 2px dotted currentColor;
}
/* Reserved: authorityClass() returns 'note-advisory' once competent_authority
   wiring lands (RFC-018 Decision 3). Kept ahead of its producer. */
.annotated-wrap :deep(mark.note-advisory) {
  border-bottom: 2px dashed currentColor;
}
.annotated-wrap :deep(mark.note-linking) {
  background: rgba(59, 130, 246, 0.28);
}
.annotated-wrap :deep(mark.note-commenting) {
  background: rgba(234, 179, 8, 0.28);
}
.annotated-wrap :deep(mark.note-questioning) {
  background: rgba(249, 115, 22, 0.3);
}
.annotated-wrap :deep(mark.note-tagging) {
  background: rgba(34, 197, 94, 0.28);
}
.annotated-wrap :deep(mark.note-other) {
  background: rgba(148, 163, 184, 0.28);
}
.annotated-wrap :deep(mark:focus-visible) {
  outline: 2px solid currentColor;
  outline-offset: 1px;
}

/* Popover card content. nldd-popover does not pad slotted content, nor
   inherit the editor UI font, so the card sets both (RijksSansVF is the
   design-system UI face). Border-left echoes the highlight colour. */
.note-pop {
  font-family: 'RijksSansVF', system-ui, sans-serif;
  padding: 14px 16px;
  border-left: 3px solid transparent;
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
  margin-bottom: 8px;
}
.note-pop__badge {
  font-size: 0.72rem;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 999px;
  color: #fff;
}
.note-pop.note-linking .note-pop__badge {
  background: #3b82f6;
}
.note-pop.note-commenting .note-pop__badge {
  background: #ca8a04;
}
.note-pop.note-questioning .note-pop__badge {
  background: #ea580c;
}
.note-pop.note-tagging .note-pop__badge {
  background: #16a34a;
}
.note-pop.note-other .note-pop__badge {
  background: #64748b;
}
.note-pop__creator {
  font-size: 0.75rem;
  opacity: 0.6;
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
