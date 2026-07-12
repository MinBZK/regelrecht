<script setup>
/**
 * NoteCreator - the note authoring form (RFC-018 write path, steps 3-5).
 *
 * Opened by AnnotatedText when the user selects text and clicks "Notitie".
 * Receives the raw [start,end) range the selection mapped to (selectionToRaw
 * already did the DOM->raw work). This component builds the TextQuoteSelector
 * (growing context until it is unique), lets the user pick a motivation and
 * fill the matching body, and emits a complete W3C Annotation object. It does
 * not persist - useDraftNotes (owned by EditorApp) does, so the new note
 * highlights live alongside committed ones.
 */
import { ref, computed, watch } from 'vue';
import { buildSelector } from '../composables/useTextSelection.js';
import { useAmbiguityVocabulary } from '../composables/useAmbiguityVocabulary.js';
import { documentsListUrl } from '../composables/corpusUrls.js';
import { apiFetchJson } from '../lib/apiFetch.js';

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
  // Active traject ref. Required to surface the "Document" link mode -
  // without a traject there is no documents folder to pick from.
  trajectRef: { type: String, default: '' },
});

const emit = defineEmits(['create', 'cancel']);

const popoverEl = ref(null);

const motivation = ref('commenting');
const creator = ref(localStorage.getItem('regelrecht-note-creator') || '');
const commentText = ref('');
const linkTarget = ref('');
// Linking sub-mode: 'element' is the existing intra-law machine_readable
// target, 'document' points at a markdown/text file in the traject's
// documents tree. Lives on the form because both produce a
// `SpecificResource` body and the W3C `motivation` is `linking` for both.
const linkMode = ref('element');
const documentTarget = ref('');
const documentOptions = ref([]);
const documentsLoadError = ref(null);
const ambiguityTag = ref('');
const workflow = ref('open');

const { items: ambiguityItems } = useAmbiguityVocabulary();

const documentsAvailable = computed(
  () => !!props.trajectRef && documentOptions.value.length > 0,
);

// Lazy-load the traject's documents the first time the form opens with
// a traject in scope. Without a traject there is no documents folder to
// link into, so skip the fetch.
async function fetchDocumentOptions() {
  if (!props.trajectRef) return;
  documentsLoadError.value = null;
  try {
    const json = await apiFetchJson(documentsListUrl(props.trajectRef), {
      errorMessage: (status) => `HTTP ${status}`,
    });
    documentOptions.value = Array.isArray(json?.documents)
      ? json.documents.map((d) => d.path)
      : [];
  } catch (e) {
    documentsLoadError.value = e;
    documentOptions.value = [];
  }
}

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
const selectorReason = computed(() => selectorResult.value?.reason ?? null);

const canSave = computed(() => {
  if (!selectorResult.value) return false;
  if (selectorStatus.value !== 'found') return false; // ambiguous/orphaned: fix first
  if (motivation.value === 'linking') {
    return linkMode.value === 'document'
      ? !!documentTarget.value
      : !!linkTarget.value;
  }
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
      // Refresh the documents list each time the form opens. The list is
      // small (a fresh traject often has zero) so a re-fetch on every open
      // is cheap and beats caching staleness across saves.
      fetchDocumentOptions().catch(() => {});
    } else {
      pop.hidePopover?.();
      // Clear the form too: range goes null on cancel/article-switch, and
      // stale commentText/linkTarget/tag would otherwise pre-fill the next
      // selection's form.
      reset();
    }
  },
);

function reset() {
  motivation.value = 'commenting'; // back to the default, not sticky on linking
  commentText.value = '';
  linkTarget.value = '';
  linkMode.value = 'element';
  documentTarget.value = '';
  ambiguityTag.value = '';
  workflow.value = 'open';
}

// True while our own cancel()/save() is inside hidePopover(), so a `close`
// dispatched synchronously from that call is recognized as self-inflicted.
// The native popover spec queues toggle (and thus nldd's close) as a task,
// which lands after the parent nulled `range` - the range guard below covers
// that regime. This flag covers the synchronous regime, so onPopoverClose
// stays correct regardless of how nldd-popover dispatches.
let selfClosing = false;

function hidePopoverSelf() {
  selfClosing = true;
  try {
    popoverEl.value?.hidePopover?.();
  } finally {
    selfClosing = false;
  }
}

function cancel() {
  hidePopoverSelf();
  reset();
  emit('cancel');
}

// nldd-popover is popover="auto": the browser light-dismisses it on an
// outside click or Esc without going through cancel(). The component fires
// `close` on every dismissal, so a close that arrives while the form is
// still open (range set) is an external dismiss and must cancel - otherwise
// the parent keeps creatorOpen=true and suppresses the "Notitie" button for
// every later selection. Closes from our own save()/cancel()/teardown are
// filtered by the selfClosing flag (synchronous dispatch) or arrive after
// the parent already nulled the range (asynchronous dispatch).
function onPopoverClose(event) {
  // Only the popover's own close counts: nldd close events bubble+composed,
  // so a future nested sheet/dialog in the form slot would land here too and
  // silently cancel a half-filled form.
  if (event.target !== event.currentTarget) return;
  if (selfClosing || !props.range) return;
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
  hidePopoverSelf();
  reset();
  emit('create', note);
}

function buildBody() {
  if (motivation.value === 'linking') {
    if (linkMode.value === 'document') {
      return {
        type: 'SpecificResource',
        // Document URI: `regelrecht://doc/<traject>/<pad>`. The
        // documentTarget already includes the in-traject relative path
        // (e.g. `mvt/concept.md`); slashes inside are kept verbatim so
        // the engine can resolve back to the file. The traject ref is
        // URI-safe by construction (`{slug}-{8hex}`) so no extra
        // encoding is needed here.
        source: `regelrecht://doc/${props.trajectRef}/${documentTarget.value}`,
        purpose: 'linking',
      };
    }
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

// One sharp line per failure reason - what went wrong + the single most
// useful fix. No bullet wall: the reason from buildSelector already
// distinguishes "too common" from "not located", so the message can be
// precise instead of listing every possibility.
const statusInfo = computed(() => {
  if (!selectorStatus.value || selectorStatus.value === 'found') return null;
  switch (selectorReason.value) {
    case 'too-common':
      return {
        title: 'Te algemeen',
        lead: 'Dit fragment komt te vaak voor om aan één plek te koppelen. Selecteer een langere, kenmerkende zinsnede.',
      };
    case 'mis-anchor':
      return {
        title: 'Niet eenduidig',
        lead: 'Dit fragment is hier niet uniek. Breid de selectie uit met de omringende woorden.',
      };
    default: // 'not-found'
      return {
        title: 'Niet teruggevonden',
        lead: 'Selecteer geen lidnummer ("1.") of opsommingsteken mee en blijf binnen één lid.',
      };
  }
});
</script>

<template>
  <nldd-popover
    ref="popoverEl"
    accessible-label="Notitie aanmaken"
    placement="bottom-start"
    width="480px"
    @close="onPopoverClose"
  >
    <div v-if="range" class="note-creator" data-testid="note-creator">
      <div class="nc-quote">
        <span class="nc-quote__label">Geselecteerd</span>
        <span class="nc-quote__text">{{ exact }}</span>
      </div>

      <!-- Unusable selection (ambiguous/orphaned): the form would only lead
           to a permanently-disabled save, so show just the warning and a way
           out. The full form appears once the selection resolves uniquely. -->
      <template v-if="statusInfo">
        <nldd-inline-dialog
          icon="warning"
          icon-color="warning"
          :text="statusInfo.title"
          :supporting-text="statusInfo.lead"
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

      <!-- Linking: choose between a machine_readable element of this
           article or a markdown/text document in the traject's
           documents folder. Document mode only shows up when an active
           traject is in scope. -->
      <label v-if="motivation === 'linking' && trajectRef" class="nc-field">
        <span class="nc-field__label">Koppel aan</span>
        <nldd-segmented-control
          size="md"
          :value="linkMode"
          @change="linkMode = $event.target?.value ?? $event.detail?.value ?? linkMode"
        >
          <nldd-segmented-control-item value="element" text="Element"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="document" text="Document"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </label>

      <label v-if="motivation === 'linking' && linkMode === 'element'" class="nc-field">
        <span class="nc-field__label">Koppel aan element</span>
        <nldd-dropdown
          size="md"
          @change="linkTarget = $event.detail?.value ?? $event.target?.value ?? linkTarget"
        >
          <select :value="linkTarget" data-testid="note-link-target">
            <option value="" disabled>Kies een element…</option>
            <option v-for="el in linkableElements" :key="el" :value="el">{{ el }}</option>
          </select>
        </nldd-dropdown>
        <span v-if="linkableElements.length === 0" class="nc-hint">
          Dit artikel heeft geen machine_readable-elementen om aan te koppelen.
        </span>
      </label>

      <label v-if="motivation === 'linking' && linkMode === 'document'" class="nc-field">
        <span class="nc-field__label">Koppel aan document</span>
        <nldd-dropdown
          size="md"
          @change="documentTarget = $event.detail?.value ?? $event.target?.value ?? documentTarget"
        >
          <select :value="documentTarget" data-testid="note-document-target">
            <option value="" disabled>Kies een document…</option>
            <option v-for="path in documentOptions" :key="path" :value="path">{{ path }}</option>
          </select>
        </nldd-dropdown>
        <span v-if="!documentsAvailable && !documentsLoadError" class="nc-hint">
          Dit traject heeft nog geen documenten. Maak er eerst een aan via Documenten.
        </span>
        <span v-if="documentsLoadError" class="nc-hint">
          Documenten kunnen niet worden geladen ({{ documentsLoadError.message }}).
        </span>
      </label>

      <!-- Comment / question body. -->
      <label v-if="motivation === 'commenting' || motivation === 'questioning'" class="nc-field">
        <span class="nc-field__label">{{ motivation === 'questioning' ? 'Vraag' : 'Toelichting' }}</span>
        <nldd-multi-line-text-field
          :value="commentText"
          rows="3"
          resize="auto"
          data-testid="note-comment-text"
          @input="commentText = $event.target?.value ?? $event.detail?.value ?? commentText"
        ></nldd-multi-line-text-field>
      </label>

      <!-- Ambiguity tag: required for tagging, optional for questioning. -->
      <label v-if="motivation === 'tagging' || motivation === 'questioning'" class="nc-field">
        <span class="nc-field__label">
          Ambiguïteit-label{{ motivation === 'questioning' ? ' (optioneel)' : '' }}
        </span>
        <nldd-dropdown
          size="md"
          @change="ambiguityTag = $event.detail?.value ?? $event.target?.value ?? ambiguityTag"
        >
          <select :value="ambiguityTag" data-testid="note-ambiguity-tag">
            <option value="">{{ motivation === 'tagging' ? 'Kies een label…' : 'Geen' }}</option>
            <option v-for="t in ambiguityItems" :key="t.id" :value="t.id">
              {{ t.label }}
            </option>
          </select>
        </nldd-dropdown>
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
  font-family: var(--primitives-font-family-body);
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
