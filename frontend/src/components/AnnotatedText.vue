<script setup>
import { computed, ref, watch, onMounted, onBeforeUnmount, nextTick } from 'vue';
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
//
// Each highlighted segment gets a stable index so the gutter card and the
// connector line can find its rendered position. A note can resolve to
// several spans; we anchor the card to the first one.
const segments = computed(() => {
  if (!props.article?.text) return [];
  return markRanges(props.article.text, props.notesForArticle);
});

// One gutter card per note (deduplicated: markRanges may emit several segments
// for one note if it has multiple spans; the card anchors to the earliest).
const noteCards = computed(() => {
  const seen = new Map();
  segments.value.forEach((seg, i) => {
    if (!seg.note) return;
    if (!seen.has(seg.note)) seen.set(seg.note, i);
  });
  return [...seen.entries()].map(([note, segIndex]) => ({ note, segIndex }));
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

function noteBodies(note) {
  return Array.isArray(note?.body) ? note.body : note?.body ? [note.body] : [];
}
function noteText(note) {
  return noteBodies(note).find((b) => b?.type === 'TextualBody' && b.purpose !== 'tagging')?.value || '';
}
function noteLink(note) {
  return noteBodies(note).find((b) => b?.type === 'SpecificResource')?.source || '';
}
function noteTags(note) {
  return noteBodies(note)
    .filter((b) => b?.type === 'TextualBody' && b.purpose === 'tagging')
    .map((b) => b.value);
}
function noteCreator(note) {
  if (!note?.creator) return '';
  return typeof note.creator === 'string' ? note.creator : note.creator.name || '';
}

// --- Layout: align each gutter card to its highlight, avoid collisions ---

const rootEl = ref(null);
const textColEl = ref(null);
const cardEls = ref([]); // card DOM nodes, set via :ref
const cardTops = ref({}); // segIndex -> px top within the gutter
const lines = ref([]); // connector polylines: { id, x1,y1, x2,y2, cls }
const hovered = ref(null); // note currently highlighted (mark or card)

function setCardRef(segIndex) {
  return (el) => {
    if (el) cardEls.value[segIndex] = el;
  };
}

// Measure highlight positions and lay the cards out beside them. Cards want to
// sit at their highlight's top; if two would overlap, the later one is pushed
// down (Hypothesis-style collision avoidance). Connector lines run from the
// right edge of the highlight to the left edge of the card.
function relayout() {
  const root = rootEl.value;
  const textCol = textColEl.value;
  if (!root || !textCol) return;
  const rootRect = root.getBoundingClientRect();

  // Desired top per card, in document order (noteCards is already ordered by
  // first segment index).
  const placements = [];
  for (const { note, segIndex } of noteCards.value) {
    const mark = textCol.querySelector(`[data-seg="${segIndex}"]`);
    if (!mark) continue;
    const r = mark.getBoundingClientRect();
    placements.push({
      segIndex,
      note,
      desiredTop: r.top - rootRect.top,
      markRight: r.right - rootRect.left,
      markMidY: r.top - rootRect.top + r.height / 2,
    });
  }

  // Resolve collisions top-down: each card starts at desiredTop but never
  // above the previous card's bottom + gap.
  const GAP = 8;
  const tops = {};
  let cursor = -Infinity;
  for (const p of placements) {
    const cardEl = cardEls.value[p.segIndex];
    const h = cardEl ? cardEl.offsetHeight : 64;
    let top = Math.max(p.desiredTop, cursor);
    tops[p.segIndex] = top;
    cursor = top + h + GAP;
    p.cardTop = top;
    p.cardHeight = h;
  }
  cardTops.value = tops;

  // Connector lines: highlight right edge -> card left edge, gutter starts
  // at the text column's right edge.
  const gutterX = textCol.offsetLeft + textCol.offsetWidth;
  lines.value = placements.map((p) => ({
    id: p.segIndex,
    note: p.note,
    x1: p.markRight,
    y1: p.markMidY,
    x2: gutterX + 4,
    y2: p.cardTop + 18,
    cls: motivationClass(p.note),
  }));
}

let ro = null;
function scheduleRelayout() {
  nextTick(() => requestAnimationFrame(relayout));
}

onMounted(() => {
  ro = new ResizeObserver(scheduleRelayout);
  if (rootEl.value) ro.observe(rootEl.value);
  scheduleRelayout();
});
onBeforeUnmount(() => {
  if (ro) ro.disconnect();
});

// Re-measure whenever the text or the resolved notes change.
watch(
  () => [props.article?.text, props.notesForArticle],
  () => {
    cardEls.value = [];
    scheduleRelayout();
  },
  { deep: true },
);
</script>

<template>
  <div v-if="article" ref="rootEl" class="annotated">
    <div ref="textColEl" class="text-col">
      <nldd-rich-text>
        <p>
          <template v-for="(seg, i) in segments" :key="i">
            <mark
              v-if="seg.note"
              :data-seg="i"
              :class="[
                motivationClass(seg.note),
                authorityClass(seg.note),
                { 'is-hovered': hovered === seg.note },
              ]"
              tabindex="0"
              @mouseenter="hovered = seg.note"
              @mouseleave="hovered = null"
              @focus="hovered = seg.note"
              @blur="hovered = null"
              >{{ seg.text }}</mark
            >
            <template v-else>{{ seg.text }}</template>
          </template>
        </p>
      </nldd-rich-text>
    </div>

    <!-- Connector lines between a highlight and its gutter card. -->
    <svg class="connectors" aria-hidden="true">
      <polyline
        v-for="ln in lines"
        :key="ln.id"
        :points="`${ln.x1},${ln.y1} ${ln.x2},${ln.y1} ${ln.x2},${ln.y2}`"
        :class="[ln.cls, { 'is-hovered': hovered === ln.note }]"
      />
    </svg>

    <!-- Gutter: one card per note, vertically aligned to its highlight. -->
    <div class="gutter">
      <nldd-card
        v-for="c in noteCards"
        :key="c.segIndex"
        :ref="setCardRef(c.segIndex)"
        class="note-card"
        :class="[motivationClass(c.note), { 'is-hovered': hovered === c.note }]"
        :style="{ top: (cardTops[c.segIndex] ?? 0) + 'px' }"
        @mouseenter="hovered = c.note"
        @mouseleave="hovered = null"
      >
        <div class="note-card__head">
          <span class="note-card__badge">{{ c.note.motivation }}</span>
          <span v-if="noteCreator(c.note)" class="note-card__creator">{{ noteCreator(c.note) }}</span>
        </div>
        <p v-if="noteText(c.note)" class="note-card__body">{{ noteText(c.note) }}</p>
        <a
          v-if="noteLink(c.note)"
          class="note-card__link"
          :href="noteLink(c.note)"
          @click.prevent
          >{{ noteLink(c.note) }}</a
        >
        <div v-if="noteTags(c.note).length" class="note-card__tags">
          <span v-for="t in noteTags(c.note)" :key="t" class="note-card__tag">{{ t }}</span>
        </div>
        <span v-if="c.note.workflow" class="note-card__workflow" :data-state="c.note.workflow">
          {{ c.note.workflow === 'open' ? 'open vraag' : 'afgehandeld' }}
        </span>
      </nldd-card>
    </div>
  </div>
  <nldd-inline-dialog v-else text="Geen artikel geselecteerd"></nldd-inline-dialog>
</template>

<style scoped>
/* Two columns: the law text on the left, a gutter of note cards on the right,
   with an SVG connector layer spanning both (Hypothesis / Google-Docs style).
   Cards are absolutely positioned by JS so they line up with their highlight
   and shift down on collision. */
.annotated {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  gap: 24px;
}
.text-col {
  min-width: 0;
}
/* Legal text carries its own paragraph breaks (\n\n); preserve them. */
.text-col :deep(p) {
  white-space: pre-wrap;
}
.gutter {
  position: relative;
}
.connectors {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  overflow: visible;
}
.connectors polyline {
  fill: none;
  stroke-width: 1.5;
  opacity: 0.4;
  transition: opacity 0.12s, stroke-width 0.12s;
}
.connectors polyline.is-hovered {
  opacity: 1;
  stroke-width: 2.5;
}
.connectors polyline.note-linking {
  stroke: #3b82f6;
}
.connectors polyline.note-commenting {
  stroke: #eab308;
}
.connectors polyline.note-questioning {
  stroke: #f97316;
}
.connectors polyline.note-tagging {
  stroke: #22c55e;
}
.connectors polyline.note-other {
  stroke: #94a3b8;
}

mark {
  padding: 0 0.1em;
  border-radius: 2px;
  cursor: default;
  transition: filter 0.12s;
}
mark.is-hovered {
  filter: brightness(1.15) saturate(1.3);
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

.note-card {
  position: absolute;
  left: 0;
  right: 0;
  display: block;
  padding: 12px 14px;
  border-left: 3px solid transparent;
  transition: box-shadow 0.12s, transform 0.12s;
}
.note-card.is-hovered {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.18);
  transform: translateX(-2px);
}
.note-card.note-linking {
  border-left-color: #3b82f6;
}
.note-card.note-commenting {
  border-left-color: #eab308;
}
.note-card.note-questioning {
  border-left-color: #f97316;
}
.note-card.note-tagging {
  border-left-color: #22c55e;
}
.note-card.note-other {
  border-left-color: #94a3b8;
}
.note-card__head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.note-card__badge {
  font-size: 0.72rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  opacity: 0.7;
}
.note-card__creator {
  font-size: 0.72rem;
  opacity: 0.55;
  margin-left: auto;
}
.note-card__body {
  margin: 0;
  font-size: 0.85rem;
  line-height: 1.45;
}
.note-card__link {
  display: inline-block;
  margin-top: 6px;
  font-size: 0.8rem;
  word-break: break-all;
}
.note-card__tags {
  margin-top: 8px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.note-card__tag {
  font-size: 0.72rem;
  padding: 1px 6px;
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.2);
}
.note-card__workflow {
  display: inline-block;
  margin-top: 8px;
  font-size: 0.72rem;
  padding: 1px 6px;
  border-radius: 4px;
}
.note-card__workflow[data-state='open'] {
  background: rgba(249, 115, 22, 0.18);
  color: #c2410c;
}
.note-card__workflow[data-state='resolved'] {
  background: rgba(34, 197, 94, 0.18);
  color: #15803d;
}

/* Narrow viewports: drop the gutter under the text instead of beside it.
   Connector lines are hidden (they only make sense in the 2-col layout). */
@media (max-width: 900px) {
  .annotated {
    grid-template-columns: 1fr;
  }
  .connectors {
    display: none;
  }
  .gutter {
    min-height: 0;
  }
  .note-card {
    position: static !important;
    margin-bottom: 8px;
  }
}
</style>
