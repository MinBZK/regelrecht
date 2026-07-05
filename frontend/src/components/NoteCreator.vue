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
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import { dismissPopoverOnScroll } from '../lib/dismissOnScroll.js';
import { buildSelector } from '../composables/useTextSelection.js';
import { useAmbiguityVocabulary } from '../composables/useAmbiguityVocabulary.js';
import { documentsListUrl } from '../composables/corpusUrls.js';
import { apiFetchJson } from '../lib/apiFetch.js';
import { useAuth } from '../composables/useAuth.js';

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
  // Active traject ref. Required to surface the "Document" link mode —
  // without a traject there is no documents folder to pick from.
  trajectRef: { type: String, default: '' },
});

const emit = defineEmits(['create', 'cancel']);

const popoverEl = ref(null);
// Cleanup for the scroll-dismiss listeners, active only while the popover is open.
let scrollCleanup = null;

// The note's author is the signed-in user — id for traceability, name for
// display. Not a free-text choice.
const { person } = useAuth();
// One flat form: the user fills any combination of a comment, an action
// status, an ambiguity tag and a link. There is no motivation picker; the
// primary motivation is derived at save time from what was filled.
const commentText = ref('');
const linkTarget = ref('');
// Link target kind: 'none' (default) leaves the note unlinked; 'element' is an
// intra-law machine_readable target, 'document' a markdown/text file in the
// traject's documents tree. Both produce a `SpecificResource` body.
const linkMode = ref('none');
const documentTarget = ref('');
const documentOptions = ref([]);
const documentsLoadError = ref(null);
const ambiguityTag = ref('');
// Action/workflow status: 'none' (default) leaves it unset; else open/resolved.
const workflow = ref('none');

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

// Ellipses signal there's more text around the fragment; omitted at the very
// start / end of the editor text so the quote reads honestly. `range` is in
// code points (see EditorView), so measure the text the same way.
const hasTextBefore = computed(() => (props.range?.start ?? 0) > 0);
const hasTextAfter = computed(() => {
  const len = [...(props.rawText ?? '')].length;
  return (props.range?.end ?? len) < len;
});
const selectorStatus = computed(() => selectorResult.value?.status ?? null);
const selectorReason = computed(() => selectorResult.value?.reason ?? null);

// Individual "is this part filled?" checks, shared by canSave and the body /
// motivation builders. A link only counts once its target is actually chosen.
const hasComment = computed(() => commentText.value.trim().length > 0);
const hasTag = computed(() => !!ambiguityTag.value);
const hasLink = computed(() =>
  linkMode.value === 'document'
    ? !!documentTarget.value
    : linkMode.value === 'element'
      ? !!linkTarget.value
      : false,
);

const canSave = computed(() => {
  if (!selectorResult.value) return false;
  if (selectorStatus.value !== 'found') return false; // ambiguous/orphaned: fix first
  // At least one piece of content — an action status alone isn't a note.
  return hasComment.value || hasTag.value || hasLink.value;
});

// The submit button stays enabled at all times; an invalid submit surfaces a
// banner instead of silently doing nothing. Clear it the moment the form
// becomes submittable so the warning never lingers after it's addressed.
const showValidationError = ref(false);
watch(canSave, (ok) => {
  if (ok) showValidationError.value = false;
});

// Show/hide the popover with the range. nldd-popover is the same primitive
// AnnotatedText uses for the hover card.
function applyRange(range) {
  const pop = popoverEl.value;
  if (!pop) return;
  if (range && props.anchor) {
    pop.anchorElement = props.anchor;
    try {
      if (!pop.matches?.(':popover-open')) pop.showPopover?.();
    } catch {
      /* already open */
    }
    // The popover is anchored to a fixed selection rect, so a background scroll
    // strands it — dismiss on scroll, same as clicking outside.
    scrollCleanup?.();
    scrollCleanup = dismissPopoverOnScroll(pop);
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
}
watch(() => props.range, applyRange);
// v-if mounts this component fresh when a note starts, so the initial range is
// already set by the time we mount — the watch alone (not immediate) would miss
// it and need a second click. Show the popover for the initial range on mount,
// when popoverEl is bound.
onMounted(() => applyRange(props.range));

function reset() {
  commentText.value = '';
  linkTarget.value = '';
  linkMode.value = 'none';
  documentTarget.value = '';
  ambiguityTag.value = '';
  workflow.value = 'none';
  showValidationError.value = false;
}

// True while our own cancel()/save() is inside hidePopover(), so a `close`
// dispatched synchronously from that call is recognized as self-inflicted.
// The native popover spec queues toggle (and thus nldd's close) as a task,
// which lands after the parent nulled `range` — the range guard below covers
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
// still open (range set) is an external dismiss and must cancel — otherwise
// the parent keeps creatorOpen=true and suppresses the "Notitie" button for
// every later selection. Closes from our own save()/cancel()/teardown are
// filtered by the selfClosing flag (synchronous dispatch) or arrive after
// the parent already nulled the range (asynchronous dispatch).
function onPopoverClose(event) {
  // Only the popover's own close counts: nldd close events bubble+composed,
  // so a future nested sheet/dialog in the form slot would land here too and
  // silently cancel a half-filled form.
  if (event.target !== event.currentTarget) return;
  // Any genuine close tears down the scroll listeners, self-close or not.
  scrollCleanup?.();
  scrollCleanup = null;
  if (selfClosing || !props.range) return;
  reset();
  emit('cancel');
}

onBeforeUnmount(() => {
  scrollCleanup?.();
  scrollCleanup = null;
});

function save() {
  if (!canSave.value) {
    showValidationError.value = true;
    return;
  }
  showValidationError.value = false;
  const note = {
    type: 'Annotation',
    motivation: primaryMotivation(),
    target: {
      source: `regelrecht://${props.lawId}`,
      selector: selectorResult.value.selector,
    },
    body: buildBody(),
  };
  const p = person.value;
  if (p) {
    note.creator = { id: p.id, name: p.name };
  }
  // Action status is optional; only attach a concrete open/resolved value.
  if (workflow.value !== 'none') {
    note.workflow = workflow.value;
  }
  note.__draft = true;
  hidePopoverSelf();
  reset();
  emit('create', note);
}

// No type picker any more: the primary W3C motivation reflects the main piece
// of content the note carries, in priority order comment > tag > link.
function primaryMotivation() {
  if (hasComment.value) return 'commenting';
  if (hasTag.value) return 'tagging';
  if (hasLink.value) return 'linking';
  return 'commenting';
}

// A note can combine parts, so the body is whatever was filled: the comment,
// the ambiguity tag and/or the link. One part returns a single Body; several
// return an array — both are valid per the annotation schema.
function buildBody() {
  const bodies = [];
  if (hasComment.value) {
    bodies.push({
      type: 'TextualBody',
      value: commentText.value.trim(),
      purpose: 'commenting',
      format: 'text/plain',
      language: 'nl',
    });
  }
  if (hasTag.value) {
    bodies.push({ type: 'TextualBody', value: ambiguityTag.value, purpose: 'tagging' });
  }
  if (linkMode.value === 'document' && documentTarget.value) {
    // Document URI: `regelrecht://doc/<traject>/<pad>`. documentTarget already
    // includes the in-traject relative path (e.g. `mvt/concept.md`); slashes
    // inside are kept verbatim so the engine can resolve back to the file. The
    // traject ref is URI-safe by construction (`{slug}-{8hex}`).
    bodies.push({
      type: 'SpecificResource',
      source: `regelrecht://doc/${props.trajectRef}/${documentTarget.value}`,
      purpose: 'linking',
    });
  } else if (linkMode.value === 'element' && linkTarget.value) {
    bodies.push({
      type: 'SpecificResource',
      source: `regelrecht://${props.lawId}/${linkTarget.value}#${linkTarget.value}`,
      purpose: 'linking',
    });
  }
  return bodies.length === 1 ? bodies[0] : bodies;
}

// One sharp line per failure reason — what went wrong + the single most
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
    <nldd-container v-if="range" padding="16" data-testid="note-creator">
      <!-- Selected fragment as an italic inline quote; ellipses show there is
           more text on either side, omitted at the very start / end. -->
      <nldd-rich-text>
        <p><i>{{ hasTextBefore ? '… ' : '' }}{{ exact }}{{ hasTextAfter ? ' …' : '' }}</i></p>
      </nldd-rich-text>

      <!-- Unusable selection (ambiguous/orphaned): show just the warning; the
           user dismisses by clicking outside. The full form appears once the
           selection resolves uniquely. -->
      <template v-if="statusInfo">
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-inline-dialog
          icon="warning"
          icon-color="warning"
          :text="statusInfo.title"
          :supporting-text="statusInfo.lead"
          data-testid="note-creator-status"
        ></nldd-inline-dialog>
      </template>

      <template v-else>
        <!-- Submit with nothing filled surfaces this banner instead of a
             disabled button, so the primary action always stays clickable. -->
        <template v-if="showValidationError">
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-banner
            variant="critical"
            text="Vul iets in of maak een keuze om een notitie toe te voegen."
            data-testid="note-validation-error"
          ></nldd-banner>
        </template>

        <nldd-spacer size="16"></nldd-spacer>
        <!-- 1. Opmerking (comment body). -->
        <nldd-form-field label="Opmerking">
          <nldd-multi-line-text-field
            :value="commentText"
            rows="3"
            resize="auto"
            data-testid="note-comment-text"
            @input="commentText = $event.target?.value ?? $event.detail?.value ?? commentText"
          ></nldd-multi-line-text-field>
        </nldd-form-field>

        <!-- 2. Actie (workflow status). Geen is the default and leaves it unset. -->
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-form-field label="Actie">
          <nldd-segmented-control
            size="md"
            accessible-label="Actie"
            :value="workflow"
            @change="workflow = $event.target?.value ?? $event.detail?.value ?? workflow"
          >
            <nldd-segmented-control-item value="none" text="Geen"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="open" text="Open"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="resolved" text="Afgehandeld"></nldd-segmented-control-item>
          </nldd-segmented-control>
        </nldd-form-field>

        <!-- 3. Ambiguïteit-label. Geen is the default empty choice. -->
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-form-field label="Ambiguïteit-label">
          <nldd-dropdown
            size="md"
            @change="ambiguityTag = $event.detail?.value ?? $event.target?.value ?? ambiguityTag"
          >
            <select :value="ambiguityTag" data-testid="note-ambiguity-tag">
              <option value="">Geen</option>
              <option v-for="t in ambiguityItems" :key="t.id" :value="t.id">
                {{ t.label }}
              </option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <!-- 4. Koppel aan. Geen (default) leaves the note unlinked; Element
             targets an intra-law machine_readable element, Document a traject
             document (only offered when a traject is in scope). -->
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-form-field label="Koppel aan">
          <nldd-segmented-control
            size="md"
            accessible-label="Koppel aan"
            :value="linkMode"
            @change="linkMode = $event.target?.value ?? $event.detail?.value ?? linkMode"
          >
            <nldd-segmented-control-item value="none" text="Geen"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="element" text="Element"></nldd-segmented-control-item>
            <nldd-segmented-control-item v-if="trajectRef" value="document" text="Document"></nldd-segmented-control-item>
          </nldd-segmented-control>
        </nldd-form-field>

        <template v-if="linkMode === 'element'">
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-form-field label="Koppel aan element">
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
          </nldd-form-field>
        </template>

        <template v-if="linkMode === 'document'">
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-form-field label="Koppel aan document">
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
          </nldd-form-field>
        </template>

        <!-- Author is the signed-in user; no field. Full-width primary action;
             the user cancels by clicking outside the popover. The button never
             disables — an empty submit shows the banner above instead. -->
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-form-actions>
          <nldd-button
            size="md"
            variant="primary"
            width="full"
            text="Voeg notitie toe"
            data-testid="note-save"
            @click="save"
          ></nldd-button>
        </nldd-form-actions>
      </template>
    </nldd-container>
  </nldd-popover>
</template>

<style scoped>
.nc-hint {
  font-size: 0.74rem;
  opacity: 0.6;
}
/* The type picker is the widest control; span the field so the four labels
   show in full instead of ellipsising. */
nldd-form-field nldd-segmented-control {
  width: 100%;
}
</style>
