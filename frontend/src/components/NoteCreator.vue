<script setup>
/**
 * NoteCreator — the note authoring form (RFC-018 write path, steps 3-5).
 *
 * Opened by AnnotatedText when the user selects text and clicks "Notitie".
 * Receives the raw [start,end) range the selection mapped to (selectionToRaw
 * already did the DOM->raw work). This component builds the TextQuoteSelector
 * (growing context until it is unique), lets the user pick a motivation and
 * fill the matching body, and emits a complete W3C Annotation object. It does
 * not persist — useDraftNotes (owned by EditorApp) does, so the new note
 * highlights live alongside committed ones.
 */
import { ref, computed, watch } from 'vue';
import { buildSelector } from '../composables/useTextSelection.js';
import { useAmbiguityVocabulary } from '../composables/useAmbiguityVocabulary.js';

const props = defineProps({
  // Raw char range from selectionToRawRange(), or null when closed.
  range: { type: Object, default: null },
  rawText: { type: String, default: '' },
  lawId: { type: String, default: '' },
  // The selected article object, for the linking target picker.
  article: { type: Object, default: null },
  // Loaded WASM engine (resolveNote) for selector uniqueness validation.
  engine: { type: Object, default: null },
  // Anchor element for the popover (the <mark>-less selection rectangle is
  // gone once the form opens, so AnnotatedText passes a stable anchor).
  anchor: { type: Object, default: null },
});

const emit = defineEmits(['create', 'cancel']);

const popoverEl = ref(null);

const motivation = ref('commenting');
const creator = ref(localStorage.getItem('regelrecht-note-creator') || '');
const commentText = ref('');
const linkTarget = ref('');
const ambiguityTag = ref('');
const workflow = ref('open');

const { items: ambiguityItems } = useAmbiguityVocabulary();

// Flatten the article's machine_readable element names so a linking note can
// point at one. Outputs and definitions are the link-worthy elements (RFC-018
// linking example targets #hoogte_zorgtoeslag, an output).
const linkableElements = computed(() => {
  const mr = props.article?.machine_readable;
  if (!mr) return [];
  const names = new Set();
  for (const o of mr.execution?.output ?? []) {
    if (o?.name) names.add(o.name);
  }
  for (const key of Object.keys(mr.definitions ?? {})) names.add(key);
  return [...names];
});

// Selector for the current selection, recomputed when the range changes.
// buildSelector grows prefix/suffix until the resolver finds it uniquely;
// `status` drives the warning shown below the quote.
const selectorResult = computed(() => {
  if (!props.range || !props.rawText || !props.engine || !props.lawId) {
    return null;
  }
  return buildSelector(
    props.rawText,
    props.range,
    props.lawId,
    props.engine,
    props.article?.number,
  );
});

const exact = computed(() => selectorResult.value?.exact ?? '');
const selectorStatus = computed(() => selectorResult.value?.status ?? null);

const canSave = computed(() => {
  if (!selectorResult.value) return false;
  if (selectorStatus.value !== 'found') return false; // ambiguous/orphaned: fix first
  if (motivation.value === 'linking') return !!linkTarget.value;
  if (motivation.value === 'commenting') return commentText.value.trim().length > 0;
  if (motivation.value === 'questioning') return commentText.value.trim().length > 0;
  if (motivation.value === 'tagging') return !!ambiguityTag.value;
  return false;
});

// Show/hide the popover with the range. nldd-popover is the same primitive
// AnnotatedText uses for the hover card.
watch(
  () => props.range,
  (range) => {
    const pop = popoverEl.value;
    if (!pop) return;
    if (range && props.anchor) {
      pop.anchorElement = props.anchor;
      try {
        if (!pop.matches?.(':popover-open')) pop.showPopover?.();
      } catch {
        /* already open */
      }
    } else {
      pop.hidePopover?.();
    }
  },
);

function reset() {
  commentText.value = '';
  linkTarget.value = '';
  ambiguityTag.value = '';
  workflow.value = 'open';
}

function cancel() {
  popoverEl.value?.hidePopover?.();
  reset();
  emit('cancel');
}

function save() {
  if (!canSave.value) return;
  const note = {
    type: 'Annotation',
    motivation: motivation.value,
    target: {
      source: `regelrecht://${props.lawId}`,
      selector: selectorResult.value.selector,
    },
    body: buildBody(),
  };
  const who = creator.value.trim();
  if (who) {
    note.creator = who;
    localStorage.setItem('regelrecht-note-creator', who);
  }
  if (motivation.value === 'questioning') {
    note.workflow = workflow.value;
  }
  note.__draft = true;
  popoverEl.value?.hidePopover?.();
  reset();
  emit('create', note);
}

function buildBody() {
  if (motivation.value === 'linking') {
    return {
      type: 'SpecificResource',
      source: `regelrecht://${props.lawId}/${linkTarget.value}#${linkTarget.value}`,
      purpose: 'linking',
    };
  }
  if (motivation.value === 'tagging') {
    return {
      type: 'TextualBody',
      value: ambiguityTag.value,
      purpose: 'tagging',
    };
  }
  const text = {
    type: 'TextualBody',
    value: commentText.value.trim(),
    purpose: motivation.value,
    format: 'text/plain',
    language: 'nl',
  };
  // A questioning note over an open norm carries both the question text and
  // the controlled ambiguity tag (RFC-018 Decision 9), so the body is a list.
  if (motivation.value === 'questioning' && ambiguityTag.value) {
    return [text, { type: 'TextualBody', value: ambiguityTag.value, purpose: 'tagging' }];
  }
  return text;
}

const statusMessage = computed(() => {
  // 'ambiguous' here covers two cases buildSelector cannot tell apart from
  // the caller's side: the quote genuinely repeats verbatim, or the resolver
  // anchored a unique/fuzzy match somewhere other than the selection (so it
  // was rejected as a mis-anchor). Both are fixed the same way — pick a
  // longer, verbatim fragment — so the message covers both without
  // over-claiming which it is.
  if (selectorStatus.value === 'ambiguous') {
    return 'Deze selectie kon niet eenduidig op de gekozen tekst worden vastgepind (komt vaker voor of week net af). Selecteer een langer fragment dat exact zo in de wettekst staat.';
  }
  if (selectorStatus.value === 'orphaned') {
    return 'De resolver vindt deze selectie niet terug. Selecteer tekst die exact in de wettekst staat.';
  }
  return '';
});

defineExpose({ popoverEl });
</script>

<template>
  <nldd-popover
    ref="popoverEl"
    accessible-label="Notitie aanmaken"
    placement="bottom-start"
    width="480px"
  >
    <div v-if="range" class="note-creator" data-testid="note-creator">
      <div class="nc-quote">
        <span class="nc-quote__label">Geselecteerd</span>
        <span class="nc-quote__text">{{ exact }}</span>
      </div>

      <!-- Unusable selection (ambiguous/orphaned): the form would only lead
           to a permanently-disabled save, so show just the warning and a way
           out. The full form appears once the selection resolves uniquely. -->
      <template v-if="statusMessage">
        <nldd-inline-dialog
          icon="warning"
          icon-color="warning"
          text="Selectie niet bruikbaar"
          :supporting-text="statusMessage"
          data-testid="note-creator-status"
        ></nldd-inline-dialog>
        <div class="nc-actions">
          <nldd-button size="md" text="Annuleren" data-testid="note-cancel" @click="cancel"></nldd-button>
        </div>
      </template>

      <template v-else>
      <label class="nc-field">
        <span class="nc-field__label">Type</span>
        <nldd-segmented-control
          size="md"
          :value="motivation"
          @change="motivation = $event.target?.value ?? $event.detail?.value ?? motivation"
        >
          <nldd-segmented-control-item value="commenting" text="Toelichting"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="linking" text="Koppeling"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="questioning" text="Vraag"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="tagging" text="Label"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </label>

      <!-- Linking: pick a machine_readable element of this article. -->
      <label v-if="motivation === 'linking'" class="nc-field">
        <span class="nc-field__label">Koppel aan element</span>
        <select
          class="nc-select"
          :value="linkTarget"
          data-testid="note-link-target"
          @change="linkTarget = $event.target.value"
        >
          <option value="" disabled>Kies een element…</option>
          <option v-for="el in linkableElements" :key="el" :value="el">{{ el }}</option>
        </select>
        <span v-if="linkableElements.length === 0" class="nc-hint">
          Dit artikel heeft geen machine_readable-elementen om aan te koppelen.
        </span>
      </label>

      <!-- Comment / question body. -->
      <label v-if="motivation === 'commenting' || motivation === 'questioning'" class="nc-field">
        <span class="nc-field__label">{{ motivation === 'questioning' ? 'Vraag' : 'Toelichting' }}</span>
        <textarea
          class="nc-textarea"
          rows="3"
          :value="commentText"
          data-testid="note-comment-text"
          @input="commentText = $event.target.value"
        ></textarea>
      </label>

      <!-- Ambiguity tag: required for tagging, optional for questioning. -->
      <label v-if="motivation === 'tagging' || motivation === 'questioning'" class="nc-field">
        <span class="nc-field__label">
          Ambiguïteit-label{{ motivation === 'questioning' ? ' (optioneel)' : '' }}
        </span>
        <select
          class="nc-select"
          :value="ambiguityTag"
          data-testid="note-ambiguity-tag"
          @change="ambiguityTag = $event.target.value"
        >
          <option value="">{{ motivation === 'tagging' ? 'Kies een label…' : 'Geen' }}</option>
          <option v-for="t in ambiguityItems" :key="t.id" :value="t.id">
            {{ t.label }}
          </option>
        </select>
      </label>

      <!-- Workflow only applies to questioning notes (RFC-018 Decision 6). -->
      <label v-if="motivation === 'questioning'" class="nc-field">
        <span class="nc-field__label">Status</span>
        <nldd-segmented-control
          size="md"
          :value="workflow"
          @change="workflow = $event.target?.value ?? $event.detail?.value ?? workflow"
        >
          <nldd-segmented-control-item value="open" text="Open"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="resolved" text="Afgehandeld"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </label>

      <label class="nc-field">
        <span class="nc-field__label">Auteur (optioneel)</span>
        <nldd-text-field
          size="md"
          :value="creator"
          data-testid="note-creator-field"
          @input="creator = $event.target?.value ?? $event.detail?.value ?? creator"
        ></nldd-text-field>
      </label>

      <div class="nc-actions">
        <nldd-button size="md" text="Annuleren" data-testid="note-cancel" @click="cancel"></nldd-button>
        <nldd-button
          size="md"
          variant="primary"
          text="Notitie toevoegen"
          data-testid="note-save"
          :disabled="!canSave || undefined"
          @click="save"
        ></nldd-button>
      </div>
      </template>
    </div>
  </nldd-popover>
</template>

<style scoped>
.note-creator {
  font-family: 'RijksSansVF', system-ui, sans-serif;
  padding: 16px;
  /* The popover host owns the width (set via the `width="480px"` attribute on
     <nldd-popover>, which it reflects to --components-popover-default-width;
     its default is only 320px, too narrow for the 4-button type control).
     The slotted card just fills that, so a child width here would only fight
     the host. box-sizing keeps the 16px padding inside the 480px. */
  box-sizing: border-box;
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.nc-quote {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
  background: rgba(148, 163, 184, 0.16);
  border-radius: 4px;
}
.nc-quote__label {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  opacity: 0.6;
}
.nc-quote__text {
  font-size: 0.88rem;
  font-style: italic;
  /* A long selected fragment must wrap inside the card, not force it wider
     than max-width or get clipped. */
  overflow-wrap: anywhere;
}
.nc-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.nc-field__label {
  font-size: 0.78rem;
  font-weight: 600;
}
/* The type picker is the widest control; let it span the (now content-sized)
   card so the four labels show in full instead of ellipsising. */
.nc-field nldd-segmented-control {
  width: 100%;
}
.nc-select,
.nc-textarea {
  font: inherit;
  padding: 6px 8px;
  border: 1px solid rgba(15, 23, 42, 0.25);
  border-radius: 4px;
  background: var(--nldd-color-surface, #fff);
  color: inherit;
}
.nc-textarea {
  resize: vertical;
  min-height: 60px;
}
.nc-hint {
  font-size: 0.74rem;
  opacity: 0.6;
}
.nc-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 4px;
}
</style>
