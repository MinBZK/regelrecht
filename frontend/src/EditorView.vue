<script setup>
import { ref, computed, reactive, watch, watchEffect, nextTick, onBeforeUnmount } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import * as yaml from 'js-yaml';
import { useLaw, fetchLaw } from './composables/useLaw.js';
import { useCorpusLaws } from './composables/useCorpusLaws.js';
import { useEngine } from './composables/useEngine.js';
import { useAuth } from './composables/useAuth.js';
import { useTrajects } from './composables/useTrajects.js';
import { useFeatureFlags } from './composables/useFeatureFlags.js';
import { useTaskActions } from './composables/useTasks.js';
import { useTaskReview } from './composables/useTaskReview.js';
import { useNotes, useResolvedDraftNotes } from './composables/useNotes.js';
import { useDraftNotes } from './composables/useDraftNotes.js';
import { lastHomePath, homeTarget } from './composables/useLastVisitedRoute.js';
import {
  registerSearchPopover,
  setEditorChrome,
  registerTabActions,
  setEditorChanges,
  registerEditorActions,
  clearEditorChrome,
} from './composables/useAppChrome.js';
import { SUPPORT_EMAIL } from './constants.js';
import { apiFetch, apiFetchJson } from './lib/apiFetch.js';
import { RETRY_MIN_SPINNER_MS } from './lib/retryFeedback.js';
import { humanizeLawId } from './lib/lawName.js';
import { quoteContext } from './lib/quoteContext.js';
import { useLatest } from './lib/useLatest.js';
import { proposalDivergence } from './lib/taskReview.js';
import ArticleText from './components/ArticleText.vue';
import ArticleTextEditor from './components/ArticleTextEditor.vue';
import NoteCreator from './components/NoteCreator.vue';
import NoteCard from './components/NoteCard.vue';
import QuotedFragment from './components/QuotedFragment.vue';
import { cpToUtf16 } from './composables/useNotesHighlight.js';
import { utf16ToCp } from './composables/useTextSelection.js';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import SearchPopover from './components/SearchPopover.vue';
import MachineReadable from './components/MachineReadable.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import ExecutionTraceView from './components/ExecutionTraceView.vue';
import LawGraphView from './components/LawGraphView.vue';

const { authenticated, oidcConfigured } = useAuth();
const { isEnabled } = useFeatureFlags();

// Per-pane view selection. Each pane independently picks one of the
// available views (Tekst, Machine, Scenario's, YAML). Same view can be
// in multiple panes (e.g. two YAML panes for diff). Layout always has
// one pane per view; nldd-side-by-side-split-view auto-hides panes
// from the right when the viewport is too narrow, so left = highest
// priority. Visibility flags via panel.* feature flags filter what's
// pickable in the menu.
const VIEW_DEFINITIONS = [
  { id: 'text', flag: 'panel.article_text', label: 'Tekst' },
  { id: 'machine', flag: 'panel.machine_readable', label: 'Machine' },
  { id: 'scenario', flag: 'panel.scenario_form', label: "Scenario's" },
  { id: 'yaml', flag: 'panel.yaml_editor', label: 'YAML' },
  { id: 'notes', flag: 'panel.notes', label: 'Notities' },
];

const availableViews = computed(() => VIEW_DEFINITIONS.filter(v => isEnabled(v.flag)));

function viewLabel(viewId) {
  return VIEW_DEFINITIONS.find(v => v.id === viewId)?.label ?? viewId;
}

const PANE_VIEWS_KEY = 'regelrecht-pane-views';
const paneViews = ref(loadPaneViews());

function loadPaneViews() {
  try {
    const stored = JSON.parse(localStorage.getItem(PANE_VIEWS_KEY) ?? 'null');
    // Only accept entries we still recognise - a stale value left over
    // from a removed view (e.g. 'form' before scenario was renamed) or
    // an externally injected string would otherwise produce a pane with
    // no v-if branch matching it, briefly flashing an empty body before
    // the availableViews watcher corrects it on the next tick.
    const knownIds = new Set(VIEW_DEFINITIONS.map(v => v.id));
    if (
      Array.isArray(stored) &&
      stored.length > 0 &&
      stored.every(v => typeof v === 'string' && knownIds.has(v))
    ) {
      return stored;
    }
  } catch {
    /* fall through to default */
  }
  return VIEW_DEFINITIONS.map(v => v.id);
}

// Every paneViews mutation goes through `paneViews.value = next`
// (setPaneView, the availableViews sync watcher), so the top-level ref
// identity always changes. No deep:true needed; in-place mutations are
// not used and shouldn't be relied on.
watch(paneViews, (val) => {
  localStorage.setItem(PANE_VIEWS_KEY, JSON.stringify(val));
});

// Sync paneViews with availableViews when a flag flips:
// - Drop panes whose view is no longer available (flag off → pane gone)
// - Append a pane for any view that was JUST enabled by this change
//   (in current available, not in previous) - so re-enabling a flag
//   brings the pane back instead of silently leaving the user without
//   it. The previous version compared "missing from paneViews" against
//   the full available set, which spuriously appended every available
//   view a user happened not to have open whenever any unrelated flag
//   flipped.
// When everything is filtered out, reset to one pane per available view
// as the safe default.
watch(availableViews, (views, oldViews) => {
  const allowedIds = new Set(views.map(v => v.id));
  const filtered = paneViews.value.filter(v => allowedIds.has(v));
  // On the very first run (immediate: true) oldViews is undefined; treat
  // it as "no diff" so we don't append every available view on top of
  // whatever the user had restored from localStorage.
  const previousIds = new Set((oldViews ?? []).map(v => v.id));
  const newlyEnabled = views
    .filter(v => oldViews !== undefined && !previousIds.has(v.id))
    .map(v => v.id);
  const next = [...filtered, ...newlyEnabled];
  if (next.length === 0 && views.length > 0) {
    paneViews.value = views.map(v => v.id);
    return;
  }
  if (
    next.length !== paneViews.value.length ||
    next.some((v, i) => v !== paneViews.value[i])
  ) {
    paneViews.value = next;
  }
}, { immediate: true });

function setPaneView(idx, viewId) {
  const next = [...paneViews.value];
  next[idx] = viewId;
  paneViews.value = next;
}

// Reorder this pane within the left→right pane order. `target`:
// 'left' | 'right' | 'start' | 'end'. Persists (paneViews → localStorage) and
// survives flag flips (the availableViews sync filter preserves order).
function movePane(idx, target) {
  const next = [...paneViews.value];
  const [moved] = next.splice(idx, 1);
  const to =
    target === 'left' ? idx - 1
      : target === 'right' ? idx + 1
        : target === 'start' ? 0
          : next.length; // 'end'
  next.splice(Math.max(0, Math.min(next.length, to)), 0, moved);
  paneViews.value = next;
}

// All edit operations are gated behind SSO + an active traject. The
// editor renders in two shapes:
//   - `/editor/{lawId?}/{articleNumber?}`         → read-only view,
//     `canEdit` is false; save buttons disabled.
//   - `/editor/{trajectRef}/{lawId?}/{article?}`  → full edit mode,
//     `canEdit` is true; writes land in that traject's branch.
// Pick a traject in the TrajectMenu to flip from the first shape to
// the second.
const { activeTrajectRef, activeTraject, trajectMissing } = useTrajects();
const canEdit = computed(
  () => (!oidcConfigured.value || authenticated.value) && activeTrajectRef.value !== null,
);
// The Tekst editor pane is editable whenever the user has write access. The
// old `editor.article_text_edit` flag that gated this separately is merged into
// the pane itself - the pane IS the editable text view, so this now just tracks
// canEdit (kept as a named alias for the text-edit affordances below).
const canEditArticleText = computed(() => canEdit.value);

const route = useRoute();
const router = useRouter();

// Bibliotheek tab / "naar bibliotheek" buttons: restore the last library
// position but re-stamp it with the currently active traject, so the
// traject survives the Editor→Bibliotheek switch (it lives in the URL).
const libraryTabTarget = computed(() => lastHomePath.value);
const libraryTabHref = computed(() => router.resolve(libraryTabTarget.value).href);

// --- Initial law load (from route params) ---
const {
  law,
  lawId,
  rawYaml,
  articles,
  lawName,
  selectedArticle,
  selectedArticleNumber,
  switchLaw,
  loading,
  error,
  saving: lawSaving,
  saveError: lawSaveError,
  saveLaw,
  currentEtag,
  lastSavedPr,
} = useLaw(route.params.lawId, route.params.articleNumber, route.params.trajectRef);

// When the active traject changes (router.push to /editor/{otherRef}/…)
// the URL stays on the same component; re-fetch the open law through the
// new traject's backends. The corpus list needs no handling here -
// `useCorpusLaws(activeTrajectRef)` re-scopes reactively on the same
// change. `switchLaw` crosses trajects too via its third argument so the
// law cache key stays correct.
//
// Also flush the WASM engine: it caches loaded laws by id only, so
// without this a scenario run after a traject switch would evaluate
// the open law against the *previous* traject's dependencies. The
// dependency walker re-loads on demand on the next run, so a single
// `unloadAllLaws` is enough - no per-dep bookkeeping needed.
watch(activeTrajectRef, (next) => {
  unloadAllLaws();
  if (lawId.value) {
    switchLaw(lawId.value, selectedArticleNumber.value, next);
  }
});

// Notes (RFC-005/RFC-018) for the current law, resolved against its text.
const {
  notesForArticle: committedNotesForArticle,
  issues: noteIssues,
  reload: reloadNotes,
} = useNotes(lawId, selectedArticle, activeTrajectRef);

// Notes render as annotations inside the editable Tekst editor (they used to
// live in a separate read-only "Tekst + notities" pane, now dropped). The
// resolver's code-point spans are converted to the editor's annotation overlay
// in `editorAnnotations`; authoring runs off the toolbar "Reactie" button.

// Note authoring (RFC-018 write path). Draft notes live in localStorage per
// law until exported; they resolve and highlight live alongside committed
// ones, so the author sees the new note anchored immediately. The WASM engine
// is handed to NoteCreator for selector-uniqueness checks; useNotes already
// initialises it, this just exposes the instance once ready (null until then,
// NoteCreator guards on it).
const noteEngine = ref(null);
useEngine()
  .initEngine()
  .then((e) => {
    noteEngine.value = e;
  })
  .catch(() => {});
const {
  drafts: draftNotes,
  draftCount,
  addDraft,
  removeDraft,
  exportYaml,
  exportYamlFromNotes,
  publishNote,
} = useDraftNotes(lawId, activeTrajectRef);
const { draftNotesForArticle } = useResolvedDraftNotes(
  draftNotes,
  lawId,
  selectedArticle,
  activeTrajectRef,
);
// Authoring is part of the notes pane (the old separate `notes.create` flag is
// folded in): wherever the pane is available, you can create notes in it.
// Note creation is a writer action (it persists to the traject sidecar), so it
// follows the same write-access gate as law-text editing. The old panel.notes
// flag is gone - notes now live in the Tekst editor itself.
const canCreateNotes = computed(() => canEdit.value);
// Committed + draft notes share the highlight path. Draft entries already
// carry __draft so the popover can mark them unsaved.
const notesForArticle = computed(() => [
  ...committedNotesForArticle.value,
  ...draftNotesForArticle.value,
]);

// A stable id per note so an annotation-click maps back to it. Committed notes
// carry a W3C id; a draft may not, so fall back to its position.
function noteKey(note, i) {
  return note?.id || `note-${i}`;
}
const noteById = computed(() => {
  const m = new Map();
  notesForArticle.value.forEach((entry, i) => m.set(noteKey(entry.note, i), entry.note));
  return m;
});
// Notes → DS annotations for the editable Tekst editor. Convert the resolver's
// code-point spans to UTF-16 offsets against the SAVED text (not editedText) so
// this stays stable while typing - the editor maps the anchors through edits
// itself. Recomputes on article switch / save, when the notes re-resolve.
const editorAnnotations = computed(() => {
  const text = selectedArticle.value?.text || '';
  const out = [];
  notesForArticle.value.forEach((entry, i) => {
    const id = noteKey(entry.note, i);
    for (const span of entry.spans) {
      const start = cpToUtf16(text, span.start);
      const end = cpToUtf16(text, span.end);
      out.push({ id, start, end, quote: text.slice(start, end) });
    }
  });
  return out;
});

// NoteCreator (authoring popover) state, opened by the toolbar comment button
// on the current editor selection.
const noteCreator = reactive({ open: false, range: null, editing: null, initialNote: null });

// - Note sheet (a right sheet hosting the badge note list and the editor) ------
// The editor (NoteCreator) is rendered as sheet content. Two entry points:
// - annotation badge → open at the 'list' view (referenced text + note cards),
//   from which editing/adding pushes the 'edit' view with a back button;
// - notities pane → open directly at 'edit' (the pane is already the list), whose
//   top bar shows a close button, not a back button.
const noteSheetEl = ref(null);
const noteSheetView = ref('list'); // 'list' | 'edit'
const noteEditFromList = ref(false); // edit entered from the in-sheet list

function openNoteEditor({ range, editing, initialNote, fromList }) {
  noteCreator.range = range;
  noteCreator.editing = editing;
  noteCreator.initialNote = initialNote;
  noteCreator.open = true;
  noteEditFromList.value = fromList;
  noteSheetView.value = 'edit';
  noteSheetEl.value?.show?.(); // no-op when already open (came from the list)
}
function resetNoteCreator() {
  noteCreator.open = false;
  noteCreator.range = null;
  noteCreator.editing = null;
  noteCreator.initialNote = null;
}
// Back button (edit → list): only offered when editing started from the list.
function noteEditorBack() {
  resetNoteCreator();
  noteSheetView.value = 'list';
}
// Cancel the editor: back to the list if that is where it came from, else close.
function noteEditCancel() {
  if (noteEditFromList.value) noteEditorBack();
  else noteSheetEl.value?.hide?.();
}
// The sheet finished closing (dismiss/Esc/backdrop or our own hide()).
function onNoteSheetClose() {
  resetNoteCreator();
  noteSheetView.value = 'list';
  activeNotes.value = [];
}
// After a create/edit, re-read the badge list's notes for the group so a new or
// edited note shows immediately.
function refreshActiveNotes(span) {
  if (!span) return;
  const g = noteGroups.value.find((gr) => gr.start === span.start && gr.end === span.end);
  if (g) activeNotes.value = [...g.notes];
}

function startNoteFromSelection(idx) {
  const refs = textEditorRefs[idx];
  if (!refs) return;
  const sel = refs.getSelection();
  if (sel.empty) return;
  const text = editedText.value || '';
  openNoteEditor({
    range: { start: utf16ToCp(text, sel.start), end: utf16ToCp(text, sel.end) },
    editing: null,
    initialNote: null,
    fromList: false,
  });
}
function onNoteCreated(note, share) {
  const span = noteCreator.range;
  const backToList = noteEditFromList.value;
  let stored;
  if (noteCreator.editing) {
    // Edit: replace the draft in place. Keep the original authorship/timestamp
    // (and id, if any); take the freshly built body/target/workflow.
    const orig = noteCreator.editing;
    const merged = { ...note, __draft: true };
    if (orig.creator) merged.creator = orig.creator;
    if (orig.created) merged.created = orig.created;
    if (orig.id) merged.id = orig.id;
    const i = draftNotes.value.indexOf(orig);
    if (i >= 0) removeDraft(i);
    stored = addDraft(merged);
  } else {
    stored = addDraft(note);
  }
  resetNoteCreator();
  // "Deel met anderen binnen dit traject" was on: commit the just-created draft
  // to the traject right away. No extra confirm - the switch was the deliberate,
  // default-off opt-in. Needs traject write access (the switch only shows then).
  if (share && stored && canEdit.value) {
    void publishOneNote(stored);
  }
  // From the badge list: return to it (refreshed). Otherwise close the sheet.
  if (backToList) {
    refreshActiveNotes(span);
    noteSheetView.value = 'list';
  } else {
    noteSheetEl.value?.hide?.();
  }
}
// "Nog een notitie" on a note group: open the editor pre-set to that group's
// existing fragment (its span, already in codepoint offsets), so the new note
// targets the same quoted text without re-selecting. `fromList` marks the
// in-sheet badge list as the origin (back returns there).
function startNoteForGroup(group, fromList = false) {
  if (group?.start == null || group?.end == null) return; // unanchored group
  openNoteEditor({
    range: { start: group.start, end: group.end },
    editing: null,
    initialNote: null,
    fromList,
  });
}
// Edit an existing draft note: open the editor pre-filled, anchored to its span.
function startEditNote(group, note, fromList = false) {
  if (group?.start == null || group?.end == null) return;
  openNoteEditor({
    range: { start: group.start, end: group.end },
    editing: note,
    initialNote: note,
    fromList,
  });
}

const activeNotes = ref([]);
// Notes grouped by the fragment they annotate (their primary span), so every
// comment on the same quote sits together under one quote header. Groups are
// ordered by where the fragment starts in the article text; a note without a
// resolvable span falls into a trailing "unanchored" group.
const noteGroups = computed(() => {
  const text = selectedArticle.value?.text || '';
  const groups = new Map();
  for (const entry of notesForArticle.value) {
    const span = entry?.spans?.[0];
    const key = span ? `${span.start}-${span.end}` : 'unanchored';
    if (!groups.has(key)) {
      // A little sentence-aware context around the fragment. Only the fragment
      // itself is italicised in the template; the context words and the ellipses
      // are not.
      const ctx = span
        ? quoteContext(text, cpToUtf16(text, span.start), cpToUtf16(text, span.end))
        : { quote: '', before: '', after: '', ellipsisBefore: '', ellipsisAfter: '' };
      groups.set(key, {
        start: span?.start ?? Number.MAX_SAFE_INTEGER,
        end: span?.end ?? null,
        ...ctx,
        notes: [],
      });
    }
    groups.get(key).notes.push(entry.note);
  }
  return [...groups.values()].sort((a, b) => a.start - b.start);
});
// A draft note lives in this browser's own localStorage, so it is the user's
// by construction - every draft is deletable. Ownership (creator.id) is only
// meaningful for committed notes in the shared repo; gating drafts on it broke
// per-note delete for pre-login drafts (no creator) and legacy drafts (whose
// creator.id was stored undefined by an earlier bug), leaving only the bulk
// wipe. Committed notes are not __draft, so they still have no delete path.
function canDeleteNote(note) {
  return !!note?.__draft;
}
function deleteNote(note) {
  const i = draftNotes.value.indexOf(note);
  if (i >= 0) removeDraft(i);
}
// Deleting a draft is irreversible (it is the only copy), so confirm first. Same
// pending-ref + show()/hide() pattern as the share modal.
const deleteModalEl = ref(null);
const deletePending = ref(null);
watch(deletePending, (note) => {
  const el = deleteModalEl.value;
  if (!el) return;
  if (note && typeof el.show === 'function') el.show();
  else if (!note && typeof el.hide === 'function') el.hide();
});
function askDeleteNote(note) {
  deletePending.value = note;
}
function cancelDelete() {
  if (deletePending.value === null) return; // idempotent: @close + button
  deletePending.value = null;
}
function confirmDelete() {
  const note = deletePending.value;
  deletePending.value = null;
  if (note) deleteNote(note);
}
// The note group (with its span) a given note belongs to, so the badge popover
// can drive an edit's re-anchoring the same way the Notities pane does.
function groupForNote(note) {
  return noteGroups.value.find((g) => g.notes.includes(note)) ?? null;
}
// The group the open badge sheet shows (all its notes share one span), so the
// sheet can offer "Notitie toevoegen" on that same fragment.
const activeGroup = computed(() =>
  activeNotes.value.length ? groupForNote(activeNotes.value[0]) : null,
);
// Sharing ("Delen") applies to your still-local drafts (a committed note is
// already on the traject branch) and needs a traject to write to.
function canPublishNote(note) {
  return !!note?.__draft && canEdit.value;
}
// Publishing writes the note to the repo (Git) and can't be undone - it can't
// be made private again - so it goes through a confirm modal. Same show()/hide()
// pattern as other DS modal dialogs: a pending ref (the note to publish) drives
// the open/close via a watch, and @close just clears it.
const publishModalEl = ref(null);
const publishPending = ref(null);
watch(publishPending, (note) => {
  const el = publishModalEl.value;
  if (!el) return;
  if (note && typeof el.show === 'function') el.show();
  else if (!note && typeof el.hide === 'function') el.hide();
});
function askPublishNote(note) {
  publishPending.value = note;
}
function cancelPublish() {
  if (publishPending.value === null) return; // idempotent: @close + button
  publishPending.value = null;
}
function confirmPublish() {
  const note = publishPending.value;
  publishPending.value = null;
  if (note) publishOneNote(note);
}
async function publishOneNote(note) {
  if (savingNotes.value) return;
  savingNotes.value = true;
  notesSaveError.value = null;
  notesSaveStatus.value = null;
  try {
    const { pr, noChange } = await publishNote(note);
    notesSaveStatus.value = noChange
      ? 'Notitie was al gedeeld.'
      : pr
        ? `Gedeeld in PR #${pr.number}.`
        : 'Notitie gedeeld.';
    await reloadNotes();
  } catch (e) {
    notesSaveError.value = e?.message || 'Delen mislukt';
  } finally {
    savingNotes.value = false;
  }
}
// Look-ahead popover shown on a badge click: a flat preview of the comments with
// a CTA into the full sheet. The DS popover positions against a real element, so
// we anchor it to the badge's viewport rect via one reused invisible element.
const annotationPopEl = ref(null);
let annotationAnchorEl = null;
// A note's comment text and author, mirroring NoteCard, for the preview.
function noteText(note) {
  const body = Array.isArray(note?.body) ? note.body[0] : note?.body;
  return body?.value || body?.source || '';
}
function noteAuthor(note) {
  return note?.creator?.name ?? note?.creator ?? '';
}
// Clicking an annotation's badge opens the look-ahead popover anchored to the
// badge; its CTA then opens the full sheet at the list view. A merged badge can
// carry several ids.
function onEditorAnnotationClick(ids, rect) {
  const idList = [...new Set(Array.isArray(ids) ? ids : [ids])];
  activeNotes.value = idList.map((id) => noteById.value.get(id)).filter(Boolean);
  if (!activeNotes.value.length) return;
  if (!rect) {
    // No rect to anchor to (e.g. keyboard activation) - skip the popover.
    openNoteListSheet();
    return;
  }
  if (!annotationAnchorEl) {
    annotationAnchorEl = document.createElement('div');
    annotationAnchorEl.setAttribute('aria-hidden', 'true');
    annotationAnchorEl.style.cssText = 'position:fixed;pointer-events:none;z-index:-1;';
    document.body.appendChild(annotationAnchorEl);
  }
  annotationAnchorEl.style.left = `${rect.left}px`;
  annotationAnchorEl.style.top = `${rect.top}px`;
  annotationAnchorEl.style.width = `${rect.width}px`;
  annotationAnchorEl.style.height = `${rect.height}px`;
  const pop = annotationPopEl.value;
  if (pop) {
    pop.anchorElement = annotationAnchorEl;
    pop.show?.();
  }
}
function openNoteListSheet() {
  noteSheetView.value = 'list';
  noteSheetEl.value?.show?.();
}
// CTA in the look-ahead popover: close it and open the full sheet.
function openSheetFromAnnotation() {
  annotationPopEl.value?.hide?.();
  openNoteListSheet();
}
onBeforeUnmount(() => {
  annotationAnchorEl?.remove();
  annotationAnchorEl = null;
});

const exporting = ref(false);
// Local YYYY-MM-DD-HH-mm-ss stamp for download filenames - hyphens throughout
// (no underscores, no colons), sortable, and unique enough to avoid clobbering
// an earlier export from the same article.
function fileTimestamp() {
  const d = new Date();
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}-${p(d.getHours())}-${p(d.getMinutes())}-${p(d.getSeconds())}`;
}

// Law id as a filename part, hyphenated (the corpus slug uses underscores).
function lawSlug() {
  return (lawId.value || 'wet').replace(/_/g, '-');
}

// Trigger a browser download of `text` as `filename`. Revoke on a later tick:
// a programmatic anchor download starts asynchronously, and revoking
// synchronously after click() races the browser fetching the blob (unreliable
// in Safari/Firefox).
function downloadYaml(text, filename) {
  const blob = new Blob([text], { type: 'text/yaml' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

// Whole-law export: every committed note (from the sidecar) plus the drafts.
async function exportNotes() {
  if (exporting.value) return; // a second click would download a duplicate
  exporting.value = true;
  try {
    downloadYaml(await exportYaml(), `${lawSlug()}-annotations-${fileTimestamp()}.yaml`);
  } finally {
    exporting.value = false;
  }
}

// Article-scoped export: only the notes that resolve to the open article
// (committed + drafts), which EditorView already has as notesForArticle.
function exportArticleNotes() {
  if (exporting.value) return;
  exporting.value = true;
  try {
    const notes = notesForArticle.value.map((e) => e.note);
    const nr = selectedArticleNumber.value || 'artikel';
    downloadYaml(exportYamlFromNotes(notes), `${lawSlug()}-artikel-${nr}-annotations-${fileTimestamp()}.yaml`);
  } finally {
    exporting.value = false;
  }
}

// Note write-back: PUT the new drafts to editor-api, which appends them to
// the sidecar on the active traject's branch (same write model as law and
// scenario edits since #632). No source picker - the traject's own corpus
// config decides where the notes land. The "PR #N" toolbar badge is driven
// by the shared lastSavedPr ref, so a save that produced a PR lights it up
// with no extra wiring here.
const savingNotes = ref(false);
const notesSaveError = ref(null);
// Explicit success signal: a PR-less / NoChange save must not look like
// the work vanished (the drafts get cleared either way).
const notesSaveStatus = ref(null);
// The save status/error describe the LAST publish. A NEW draft appearing
// afterwards makes the confirmation stale ("Notitie gedeeld" next to a fresh
// unsaved draft is contradictory), so clear it then. Sharing itself removes the
// just-shared draft - clearing on a DECREASE would wipe the very confirmation
// publishOneNote is about to set. Only react to an increase; the count going
// down is a share/delete completing.
watch(draftCount, (count, prev) => {
  if (count > prev) {
    notesSaveStatus.value = null;
    notesSaveError.value = null;
  }
});

const resultSheetOpen = ref(false);
const graphSheetOpen = ref(false);

// --- Corpus search (reuse the shared SearchPopover) ---
const searchPopoverRef = ref(null);
// The toolbar search control lives in the AppShell; register our popover so
// the shell's search button/field opens it.
registerSearchPopover(searchPopoverRef);

// Shared corpus list via `useCorpusLaws` - the same per-scope cache
// MachineReadable reads, so an editor mount fires ONE laws-list fetch
// instead of a private duplicate GET. Traject switches re-scope
// reactively; fetch failures degrade to the humanized law id.
const {
  displayName: corpusDisplayName,
  refresh: refreshCorpusLaws,
} = useCorpusLaws(activeTrajectRef);

/**
 * Display name for the failed law on the error inline-dialog. Tries the
 * corpus index first; falls back to the URL slug (via `humanizeLawId`
 * inside `displayName`) so the user always sees a concrete identifier.
 */
const failedLawName = computed(() => {
  const id = lawId.value;
  if (!id) return '';
  return corpusDisplayName(id);
});

// True when the law fetch errored with 404 (law missing or not in active traject's corpus).
const lawErrorIs404 = computed(() => error.value?.status === 404);

/**
 * Retry the failed law fetch. switchLaw clears `error` and re-runs the
 * fetch; failed responses don't enter the cache so a retry actually hits
 * the network again.
 */
function retryLoadLaw() {
  if (!lawId.value) return;
  switchLaw(lawId.value, selectedArticleNumber.value, undefined, { minLoadingMs: RETRY_MIN_SPINNER_MS });
}

function onSearchSelectLaw(lawIdVal) {
  // Open in the library - search currently only matches law names. As
  // soon as article-level search lands, we can route directly into the
  // editor (with the chosen article as the active tab).
  router.push(libraryRouteFor(lawIdVal));
}

async function onSearchHarvestAvailable(slug) {
  // Best-effort reload - a failure just means the list below may not
  // include the fresh law yet.
  await apiFetch('/api/corpus/reload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ law_ids: [slug] }),
  }).catch(() => {});
  // Bust the shared per-scope cache so the fresh law shows up with its
  // real display name instead of the humanized slug fallback.
  await refreshCorpusLaws();
  router.push(libraryRouteFor(slug));
}
const resultSheetEl = ref(null);
watch(resultSheetOpen, async (open) => {
  await nextTick();
  if (open) resultSheetEl.value?.show();
  else resultSheetEl.value?.hide();
});
const graphSheetEl = ref(null);
watch(graphSheetOpen, async (open) => {
  await nextTick();
  if (open) graphSheetEl.value?.show();
  else graphSheetEl.value?.hide();
});

// --- Multi-law tab state (persisted in localStorage) ---
const TABS_STORAGE_KEY = 'regelrecht-open-tabs';
const ACTIVE_TAB_STORAGE_KEY = 'regelrecht-active-tab';

function loadSavedTabs() {
  try {
    const saved = localStorage.getItem(TABS_STORAGE_KEY);
    const parsed = saved ? JSON.parse(saved) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch { return []; }
}

function saveTabs(tabs) {
  localStorage.setItem(TABS_STORAGE_KEY, JSON.stringify(tabs));
}

function loadSavedActiveTab() {
  try {
    const saved = localStorage.getItem(ACTIVE_TAB_STORAGE_KEY);
    return saved ? JSON.parse(saved) : null;
  } catch { return null; }
}

function saveActiveTab(tab) {
  if (!tab) localStorage.removeItem(ACTIVE_TAB_STORAGE_KEY);
  else localStorage.setItem(ACTIVE_TAB_STORAGE_KEY, JSON.stringify(tab));
}

/**
 * Build a router target for the editor that preserves the current
 * traject scope. With a traject in the URL the user stays in
 * `editor-traject` (full edit mode). EditorView only ever mounts under a
 * traject (the editor requires one), so the no-traject branch should not
 * fire here - but for safety it routes to the chooser with the law as
 * query rather than the (now removed) no-traject editor.
 */
function editorRouteFor(lawIdVal, articleNumber) {
  const trajectRef = route.params.trajectRef;
  if (trajectRef) {
    return {
      name: 'editor-traject',
      params: { trajectRef, lawId: lawIdVal, articleNumber },
    };
  }
  return {
    name: 'editor',
    query: { law: lawIdVal || undefined, article: articleNumber || undefined },
  };
}

/**
 * Build a router target for the bibliotheek that preserves the current
 * traject scope, mirroring editorRouteFor. Used when leaving the editor
 * for the library (e.g. opening a search result) so the active traject
 * follows the user instead of being dropped.
 */
function libraryRouteFor(lawIdVal) {
  return homeTarget({ trajectRef: route.params.trajectRef, lawId: lawIdVal });
}

const openTabs = ref(loadSavedTabs());

// Cache for law names (populated on fetch)
const lawNames = ref({});

// Active tab tracks which tab is selected
const activeTab = ref(null);

function tabKey(tab) {
  return `${tab.lawId}:${tab.articleNumber}`;
}

function findTab(lawIdVal, articleNumber) {
  return openTabs.value.find(t => t.lawId === lawIdVal && t.articleNumber === String(articleNumber));
}

// Add tab when initial law loads
watch([() => lawId.value, selectedArticle], ([id, article]) => {
  if (!id || !article) return;
  const num = String(article.number);
  if (!findTab(id, num)) {
    const MAX_TABS = 20;
    const tabs = [...openTabs.value, { lawId: id, articleNumber: num }];
    openTabs.value = tabs.length > MAX_TABS ? tabs.slice(-MAX_TABS) : tabs;
    saveTabs(openTabs.value);
  }
  activeTab.value = { lawId: id, articleNumber: num };
  saveActiveTab(activeTab.value);
  if (lawName.value) lawNames.value = { ...lawNames.value, [id]: lawName.value };
});

// Also populate lawNames when lawName resolves
watch(lawName, (name) => {
  if (name && lawId.value) {
    lawNames.value = { ...lawNames.value, [lawId.value]: name };
  }
});

// Shared between selectTab and the route guard below: a tab switch and a
// URL-driven switch supersede each other.
const claimTabSwitch = useLatest();

async function selectTab(tab) {
  const isCurrent = claimTabSwitch();
  activeTab.value = tab;
  // Restore snapshot if the user is mid-edit, otherwise the partial mutations
  // would persist into the new tab's view.
  if (activeAction.value) {
    handleActionClose();
  }
  if (tab.lawId === lawId.value) {
    selectedArticleNumber.value = tab.articleNumber;
  } else {
    await switchLaw(tab.lawId, tab.articleNumber);
    if (!isCurrent()) return; // stale, another switch started
    lawNames.value = { ...lawNames.value, [tab.lawId]: lawName.value };
  }
  // Sync the URL so deep-linking and browser back/forward stay in step.
  // `replace` (not `push`) keeps history clean - a tab switch isn't
  // navigation the user wants to undo with the back button.
  router.replace(editorRouteFor(tab.lawId, tab.articleNumber));
}

// On load there may be no article to edit yet - the URL carries no article
// (just a traject, or a law without an article number). Rather than show the
// empty state while tabs are still open, open one right away: the last active
// tab when the URL has no law at all (so a refresh returns the user where they
// were), otherwise simply the first open tab. selectTab sets activeTab
// synchronously, so the empty state never flashes. With no open tabs we fall
// through to it - the only case it should appear.
if (!route.params.articleNumber && openTabs.value.length > 0) {
  const lastActive = loadSavedActiveTab();
  const restored = !route.params.lawId && lastActive?.lawId
    ? findTab(lastActive.lawId, lastActive.articleNumber)
    : null;
  selectTab(restored || openTabs.value[0]).catch(console.warn);
}

// Browser back/forward (or any external navigation) - pull state from
// URL. Local mutations from selectTab already match the destination,
// so the guards below short-circuit; the work only happens for true
// URL changes.
//
// trajectRef-only changes are intentionally NOT handled here: the
// `watch(activeTrajectRef)` above already does `unloadAllLaws` +
// `switchLaw`, and triggering switchLaw twice in
// the same tick would burn an extra fetch (the first await loses
// useLaw's stale-switch race, but still hits the network). This guard
// handles the law / article portion only.
onBeforeRouteUpdate(async (to) => {
  const newLawId = to.params.lawId;
  const newArticle = to.params.articleNumber;
  if (!newLawId) return;
  if (newLawId !== lawId.value) {
    const isCurrent = claimTabSwitch();
    await switchLaw(newLawId, newArticle, to.params.trajectRef || null);
    if (!isCurrent()) return;
    lawNames.value = { ...lawNames.value, [newLawId]: lawName.value };
  } else if (newArticle && String(newArticle) !== String(selectedArticleNumber.value)) {
    selectedArticleNumber.value = String(newArticle);
  }
});

function closeTab(tab) {
  openTabs.value = openTabs.value.filter(t => tabKey(t) !== tabKey(tab));
  saveTabs(openTabs.value);
  if (activeTab.value && tabKey(activeTab.value) === tabKey(tab)) {
    const remaining = openTabs.value;
    if (remaining.length > 0) {
      selectTab(remaining[remaining.length - 1]).catch(console.warn);
    } else {
      activeTab.value = null;
    }
  }
}

function tabDisplayName(tab) {
  return lawNames.value[tab.lawId] || humanizeLawId(tab.lawId);
}

// Publish the editor-only chrome (federated "PR #N" indicator + document
// tabs) to the AppShell, which can't own these statically. Tab actions are
// registered once; the values stay in sync reactively while mounted and are
// cleared on unmount so the library never shows a stale PR badge or tab bar.
// Drag/keyboard reorder from the mobile tabs sheet. The nldd-list dispatches
// nldd-reorder with array indices; mirror the move into openTabs so the new
// order persists (and the md+ document-tab-bar follows it).
function reorderTabs(fromIndex, toIndex) {
  const tabs = [...openTabs.value];
  if (
    fromIndex < 0 || fromIndex >= tabs.length ||
    toIndex < 0 || toIndex >= tabs.length ||
    fromIndex === toIndex
  ) return;
  const [moved] = tabs.splice(fromIndex, 1);
  tabs.splice(toIndex, 0, moved);
  openTabs.value = tabs;
  saveTabs(openTabs.value);
}

registerTabActions({
  key: tabKey,
  displayName: tabDisplayName,
  select: (tab) => { selectTab(tab).catch(console.warn); },
  close: closeTab,
  reorder: reorderTabs,
});
watchEffect(() => {
  setEditorChrome({
    pr: lastSavedPr.value,
    tabs: openTabs.value,
    activeTab: activeTab.value,
  });
});
onBeforeUnmount(clearEditorChrome);

// Load lawNames for persisted tabs on startup (parallel, deduplicated).
// Reads go through the currently-active traject so tab labels match
// what the editor pane shows after a save.
const uniqueLawIds = [...new Set(openTabs.value.map(t => t.lawId))];
Promise.all(uniqueLawIds.map(async (id) => {
  try {
    const entry = await fetchLaw(activeTrajectRef.value, id);
    lawNames.value = { ...lawNames.value, [id]: entry.lawName };
  } catch { /* ignore */ }
}));

// --- Engine ---
const {
  ready: engineReady,
  initError: engineInitError,
  initEngine,
  getEngine,
  loadLawYaml,
  unloadAllLaws,
} = useEngine();
initEngine().catch(() => {});

// The engine-loading watch lives below, next to `currentLawYaml`, so it
// observes in-memory edits rather than only the persisted `rawYaml`.

// --- Trace state (receives trace from last executed scenario) ---
const lastTraceText = ref(null);
const lastResult = ref(null);
const lastError = ref(null);
const lastExpectations = ref({});
const lastScenarioName = ref('');
// The scenario's entry output (e.g. "is_rechthebbende"). The graph view
// uses this to pin its "▶ start" marker to the right leaf.
const lastOutputName = ref(null);
// Loading state of the last scenario and a bound re-run callback, so the
// result sheet can show "running…" / an error with a reload action.
const lastRunning = ref(false);
const lastReload = ref(null);

function handleScenarioExecuted({ result, traceText, error, running, expectations, scenarioName, outputName, reload, view }) {
  lastResult.value = result;
  lastTraceText.value = traceText;
  lastError.value = error || null;
  lastRunning.value = !!running;
  lastReload.value = typeof reload === 'function' ? reload : null;
  lastExpectations.value = expectations || {};
  lastScenarioName.value = scenarioName || '';
  lastOutputName.value = outputName || null;
  // Open exactly one of the two sheets - opening the second on top of
  // the first would leave the previous one as a stale layer that
  // re-appears when the user dismisses the foreground sheet.
  if (view === 'graph') {
    resultSheetOpen.value = false;
    graphSheetOpen.value = true;
  } else {
    graphSheetOpen.value = false;
    resultSheetOpen.value = true;
  }
}

// Clear the captured trace whenever the active law changes - otherwise
// LawGraphView would re-flatten the old trace under the new lawId,
// misattribute every step to the new law, and pin the "▶ start" badge
// to a leaf that just happens to share the previous output's name.
// The trace and graph sheets close along with the trace they were
// showing: leaving them open over an empty graph or a fresh law the
// user just navigated into is more confusing than auto-dismissing.
watch(lawId, () => {
  lastResult.value = null;
  lastTraceText.value = null;
  lastError.value = null;
  lastRunning.value = false;
  lastReload.value = null;
  lastExpectations.value = {};
  lastScenarioName.value = '';
  lastOutputName.value = null;
  resultSheetOpen.value = false;
  graphSheetOpen.value = false;
});

// --- Editor state ---
const activeAction = ref(null);
// True while the open action sheet is for a freshly added action, so its
// Save button is always offered (no edit required to create it).
const activeActionIsNew = ref(false);
const activeEditItem = ref(null);
const parseError = ref(null);

const machineReadable = ref(null);

// The YAML pane edits the machine-readable as text. There is something to edit
// when the article carries machine_readable, or one was created/edited this
// session (the live ref). With neither, the empty code editor is replaced by a
// message - like the Machine pane and the read-only YAML view. Deliberately not
// keyed on the live ref alone: clearing or mid-typing invalid YAML nulls it, and
// that must not yank the editor out from under the user.
const hasMachineReadable = computed(
  () => !!machineReadable.value || !!selectedArticle.value?.machine_readable,
);
const yamlSource = ref('');
// In-memory markdown for the currently selected article's `text` field.
// Seeded on article switch alongside machineReadable so the Tekst and Machine
// panes reset in lockstep when the user tabs to a different article.
const editedText = ref('');

// Per-pane refs to the ArticleTextEditor instance so the header toolbar can
// render the formatting controls (Bold/Italic/lists) next to the existing
// pane-view dropdown rather than the editor drawing its own duplicate
// label dropdown inside the body. Functional ref keeps the map in sync as
// panes mount/unmount.
const textEditorRefs = reactive({});
function setTextEditorRef(idx) {
  return (el) => {
    if (el) {
      textEditorRefs[idx] = el;
    } else {
      delete textEditorRefs[idx];
    }
  };
}

// --- Tekst-opmaak (segmented controls in de header-toolbar) ---
// De ArticleTextEditor (Tiptap) per pane is de bron van waarheid; deze helpers
// vertalen activeFormats <-> de segmented-control-waardes en sturen de
// toggle-commando's aan. Lezen van activeFormats in de template houdt de
// controls reactief in sync met de selectie.

// Vet/Schuin: checkbox-control - de geselecteerde waardes zijn de actieve
// inline-formats (beide kunnen tegelijk aan staan).
function boldItalicValues(idx) {
  const refs = textEditorRefs[idx];
  if (!refs) return [];
  const values = [];
  if (refs.activeFormats.bold) values.push('bold');
  if (refs.activeFormats.italic) values.push('italic');
  return values;
}
function onInlineFormatChange(idx, e) {
  const refs = textEditorRefs[idx];
  if (!refs) return;
  if (e.detail.value === 'bold') refs.toggleBold();
  else if (e.detail.value === 'italic') refs.toggleItalic();
}

// Lijst: radio-control met none/bullet/ordered. Tiptap's toggle-commando's
// converteren tussen lijsttypes en heffen de actieve lijst op, dus elke
// overgang is met één toggle te maken.
function listValue(idx) {
  const refs = textEditorRefs[idx];
  if (!refs) return 'none';
  if (refs.activeFormats.bulletList) return 'bullet';
  if (refs.activeFormats.orderedList) return 'ordered';
  return 'none';
}
function setList(idx, target) {
  const refs = textEditorRefs[idx];
  if (!refs) return;
  const current = listValue(idx);
  if (target === current) return;
  if (target === 'bullet') refs.toggleBulletList();
  else if (target === 'ordered') refs.toggleOrderedList();
  else if (refs.activeFormats.bulletList) refs.toggleBulletList();
  else if (refs.activeFormats.orderedList) refs.toggleOrderedList();
}
function onListChange(idx, e) {
  setList(idx, e.detail.value);
}

const dumpOpts = { lineWidth: 80, noRefs: true };

watch(selectedArticle, (article) => {
  activeAction.value = null;
  activeEditItem.value = null;
  const mr = article?.machine_readable;
  machineReadable.value = mr ? structuredClone(mr) : null;
  yamlSource.value = mr ? yaml.dump(mr, dumpOpts) : '';
  editedText.value = article?.text ?? '';
  parseError.value = null;
}, { immediate: true });

const editedArticle = computed(() => {
  if (!selectedArticle.value) return null;
  return {
    ...selectedArticle.value,
    text: editedText.value,
    machine_readable: machineReadable.value,
  };
});

// Parse rawYaml once per law load into a reusable document skeleton. The
// computed below splices in the currently edited article's
// machine_readable on every reactive change, so without this cache each
// keystroke in the YAML textarea would re-parse the whole ~25-200 KiB law
// on the main thread. Hoisting the parse to a computed keyed only on
// rawYaml drops that cost to one parse per load.
const parsedRawLaw = computed(() => {
  if (!rawYaml.value) return null;
  try {
    return yaml.load(rawYaml.value);
  } catch {
    return null;
  }
});

// Reactive "edited" law YAML: rawYaml with the currently selected article's
// machine_readable substituted in. This is what flows into the engine and
// into ScenarioBuilder, so in-memory edits re-execute scenarios without a
// round-trip through the backend.
//
// Only the currently selected article's machine_readable is swapped - edits
// on other articles are not tracked across tab switches (existing behavior
// of the editor state model).
//
// KNOWN LIMITATION: when this value is sent to `saveLaw` (via the Machine
// panel save button), the body is the `yaml.dump` output of the
// reconstructed document - which strips YAML comments and may reorder
// top-level keys compared to `rawYaml`. The YAML-pane edit path preserves
// the user's exact text via `yamlSource`, so it does not have this drift.
// Today's corpus is harvester-generated and comment-free, so the impact is
// zero in practice; revisit if hand-annotated laws are introduced (e.g.
// keep an "as-typed" base alongside `rawYaml` and only re-dump the edited
// article).
const currentLawYaml = computed(() => {
  if (!rawYaml.value) return null;
  if (!selectedArticle.value) return rawYaml.value;
  const base = parsedRawLaw.value;
  if (!base) return rawYaml.value;
  try {
    // Shallow-clone the doc and the articles array so our splice doesn't
    // mutate the memoized `parsedRawLaw` value - Vue would consider the
    // computed still fresh but the next read would see our substituted
    // article instead of the original.
    const doc = { ...base };
    const docArticles = Array.isArray(base.articles) ? [...base.articles] : null;
    if (!docArticles) return rawYaml.value;
    const idx = docArticles.findIndex(
      (a) => String(a.number) === String(selectedArticleNumber.value),
    );
    if (idx < 0) return rawYaml.value;
    // Short-circuit when neither pane has been touched: a `yaml.dump`
    // round-trip can cosmetically reformat the parsed doc (key ordering,
    // string quoting), and `handleActionSave` calls `saveLaw(currentLawYaml)`
    // without checking dirty flags, so a no-op action-modal save on a
    // pristine article would otherwise produce a reformat-only commit.
    const baseArticle = docArticles[idx];
    const textPristine = editedText.value === (baseArticle.text ?? '');
    const baselineMr = baseArticle.machine_readable ?? null;
    const currentMr = machineReadable.value ?? null;
    const mrPristine = baselineMr === currentMr
      || JSON.stringify(baselineMr) === JSON.stringify(currentMr);
    if (textPristine && mrPristine) return rawYaml.value;
    // Only splice fields that have diverged from the base - passing
    // `machineReadable.value` verbatim when it's null would erase the
    // article's machine_readable from the serialized doc, and similarly
    // for text. The dirty computeds below drive this same contract.
    const patched = { ...baseArticle };
    if (!textPristine) {
      patched.text = editedText.value;
    }
    if (machineReadable.value != null) {
      patched.machine_readable = machineReadable.value;
    } else if (baselineMr != null) {
      // The user opened an article that had machine_readable and cleared the
      // YAML editor. The spread above carried the original key over; we have
      // to drop it explicitly so the save persists the deletion rather than
      // silently round-tripping the original content.
      delete patched.machine_readable;
    }
    docArticles[idx] = patched;
    doc.articles = docArticles;
    return yaml.dump(doc, dumpOpts);
  } catch {
    return rawYaml.value;
  }
});

// Load current law into engine. Reacts to currentLawYaml so in-memory
// edits are immediately visible to scenarios. Goes through
// useEngine.loadLawYaml so the engine's scope-tracking map sees this
// load under the right traject - otherwise a later loadDependency call
// for the same law id would treat it as already-current and skip the
// refetch on a traject switch.
// `currentLawYaml` re-dumps on every keystroke, so reacting directly would
// reload the WASM engine per keystroke. Debounce the reload ~300ms after the
// last edit; keep the first load (no previous yaml) synchronous so the initial
// load isn't delayed. Subsequent transitions from an existing yaml - keystroke
// edits, article switches, traject switches - debounce.
let engineLoadDebounce = null;

async function reloadEngineLaw(lawYaml, isReady) {
  if (!isReady || !lawYaml) return;
  try {
    await loadLawYaml(lawYaml, lawId.value, activeTrajectRef.value);
  } catch (e) {
    console.warn(`Failed to load law '${lawId.value}' into engine:`, e);
  }
}

watch(
  [currentLawYaml, engineReady],
  ([lawYaml, isReady], prev) => {
    clearTimeout(engineLoadDebounce);
    // First load (no previous yaml): run immediately so scenarios see the law
    // without delay. Any change from an existing yaml debounces.
    if (!prev || !prev[0]) {
      reloadEngineLaw(lawYaml, isReady);
      return;
    }
    engineLoadDebounce = setTimeout(() => reloadEngineLaw(lawYaml, isReady), 300);
  },
  { immediate: true },
);

onBeforeUnmount(() => clearTimeout(engineLoadDebounce));

// Dirty state: the selected article's in-memory machine_readable differs
// from the article's saved copy. `machineReadable.value` starts as a deep
// JSON clone of `selectedArticle.machine_readable` (see the `watch` above),
// so for field-based edits the two share the same key order and
// `JSON.stringify` is a cheap, accurate structural comparison.
//
// Note: the YAML-pane edit path (`onYamlInput`) replaces `machineReadable`
// with a fresh `yaml.load(text)` object whose key order comes from the
// textarea, so a no-op round-trip can flip this flag to `true` even when
// the semantic content is unchanged. That's a conservative false positive
// - the worst case is an enabled save button - so we accept it rather
// than pay for a canonical YAML dump on every keystroke.
const isMachineReadableDirty = computed(() => {
  if (!selectedArticle.value) return false;
  const saved = selectedArticle.value.machine_readable ?? null;
  const current = machineReadable.value ?? null;
  if (saved == null && current == null) return false;
  try {
    return JSON.stringify(saved) !== JSON.stringify(current);
  } catch {
    return true;
  }
});

const isArticleTextDirty = computed(() => {
  if (!selectedArticle.value) return false;
  return (selectedArticle.value.text ?? '') !== (editedText.value ?? '');
});

// Tracks which pane(s) had dirty edits at the time of the most recent save
// attempt. Used to scope `lawSaveError` to the pane that actually triggered
// the save - without this, a save initiated from the machine pane that
// fails would surface the "Opslaan mislukt" dialog inside the text-pane
// body too, blaming a pane the user didn't touch.
const lastSaveTouchedText = ref(false);
const lastSaveTouchedMachine = ref(false);

// --- Review-modus (job_review-taak) -----------------------------------
// `?task=<id>`: show a job_review task's proposed law YAML as an unsaved
// edit rather than fetching it separately. Flag-independent (the taken-UI
// is GA); only requesting a new enrichment is still flag-gated. The proposal is applied to the first article where it
// diverges from the saved law (seeding `editedText`/`machineReadable`,
// the same pane-local "current" refs a manual edit would touch), so the
// existing dirty-tracking and Wijzigingenbalk (Opslaan/Wijzigingen-
// ongedaan) drive the review UI the same way they drive a manual edit.
// The SEEDING is single-article-scoped like `currentLawYaml` below (a
// proposal touching several articles only seeds the first one that
// differs into the panes), but the SAVE is not: `handleLawSave` sends the
// full `reviewProposedContent` when a review is active, so approving
// always commits the entire proposal - never just the seeded article.
const {
  reviewTask,
  proposedContent: reviewProposedContent,
  stale: reviewStale,
  loadError: reviewLoadError,
  loadReview,
  approveAfterSave,
  reject: rejectReviewInternal,
} = useTaskReview();
const reviewActive = computed(() => !!reviewTask.value);
const reviewTaskIdParam = computed(() =>
  typeof route.query.task === 'string' ? route.query.task : null,
);
// Guards against re-firing loadReview for a task id already attempted -
// approve/reject null out `reviewTask`, which would otherwise look
// indistinguishable from "not loaded yet" and re-trigger against the
// task we just resolved.
let reviewAttemptedForTaskId = null;

// Drop `?task=` from the URL once the review is resolved (approved or
// rejected) so a refresh/back-navigation doesn't re-open review mode.
// Rebuilt from the CURRENT law/article (not `route.params`, which still
// names whatever article the URL originally pointed at): applyProposedContent
// may have moved `selectedArticleNumber` to the article the proposal
// actually touches, and `onBeforeRouteUpdate` would otherwise see the
// stale route param disagree with `selectedArticleNumber` and snap the
// editor back to the pre-review article right after resolving.
function clearReviewQuery() {
  router.replace(editorRouteFor(lawId.value, selectedArticleNumber.value));
}

// Whether the proposal seeded anything (false when every proposed article
// matches the saved law, or the only differences are articles the saved
// law doesn't have - see the comment on the loop below).
const reviewSeeded = ref(false);
// Whether the proposal touches article(s) beyond the single one seeded into
// the editor panes (or, when nothing was seedable, the proposal touches
// anything at all). The visible editor is always single-article-scoped, but
// Opslaan now approves the FULL proposal regardless - this only drives the
// banner copy that points the reviewer at the YAML panel for the rest.
const reviewHasHiddenChanges = ref(false);

function applyProposedContent(proposedYaml) {
  reviewSeeded.value = false;
  reviewHasHiddenChanges.value = false;
  let proposed;
  try {
    proposed = yaml.load(proposedYaml);
  } catch {
    return; // malformed proposal - leave the saved content in place
  }
  const proposedArticles = Array.isArray(proposed?.articles) ? proposed.articles : [];
  // v1 can only seed an EXISTING article as an unsaved edit (same
  // single-article model as `currentLawYaml`'s KNOWN LIMITATION below,
  // which has no way to splice in an article the saved law doesn't have),
  // and can't show a removed article either - proposalDivergence folds both
  // into `hiddenChanges` so the banner points at the YAML panel for them.
  const { target, hiddenChanges } = proposalDivergence(articles.value, proposedArticles);
  reviewHasHiddenChanges.value = hiddenChanges;
  if (!target) return; // nothing seedable differs - nothing to seed
  reviewSeeded.value = true;
  selectedArticleNumber.value = String(target.number);
  // `watch(selectedArticle)` (above) resets editedText/machineReadable to
  // the (still-saved) newly selected article; wait a tick so the seed
  // below lands after that reset instead of being clobbered by it.
  nextTick(() => {
    const mr = target.machine_readable ?? null;
    machineReadable.value = mr ? structuredClone(mr) : null;
    yamlSource.value = mr ? yaml.dump(mr, dumpOpts) : '';
    editedText.value = target.text ?? '';
  });
}

// Banner copy for the review-mode dialog. Opslaan always approves the FULL
// proposal (see `handleLawSave`), so the copy says so explicitly; when the
// proposal touches more than the single seeded article (or nothing could be
// seeded at all), point the reviewer at the YAML panel (`panel.yaml_editor`)
// for the parts the article-scoped editor can't show.
const REVIEW_HIDDEN_CHANGES_NOTE =
  'Het voorstel kan ook artikelen wijzigen die hier niet zichtbaar zijn — bekijk het YAML-paneel voor het geheel.';
const reviewSupportingText = computed(() => {
  if (reviewLoadError.value) return reviewLoadError.value;
  if (!reviewSeeded.value) {
    return (
      'Dit voorstel wijkt niet af van een bestaand artikel dat de editor hier kan tonen, ' +
      'of raakt alleen artikelen die hier niet zichtbaar zijn. Beoordeel het handmatig via ' +
      `het YAML-paneel. ${REVIEW_HIDDEN_CHANGES_NOTE} Klik op "Voorstel opslaan en ` +
      'goedkeuren" om het volledige voorstel te accepteren, of verwerp de taak.'
    );
  }
  if (reviewStale.value) {
    return (
      'Let op: de wet is gewijzigd sinds deze verrijking draaide. Controleer het voorstel extra goed.' +
      (reviewHasHiddenChanges.value ? ` ${REVIEW_HIDDEN_CHANGES_NOTE}` : '')
    );
  }
  return (
    'Opslaan keurt het volledige voorstel goed (de hele wet); Verwerpen wijst het af. ' +
    'Handmatige aanpassingen worden daarbij niet meegenomen — wil je zelf bewerken, ' +
    'verwerp dan eerst het voorstel.' +
    (reviewHasHiddenChanges.value ? ` ${REVIEW_HIDDEN_CHANGES_NOTE}` : '')
  );
});

// Review-modus is flag-onafhankelijk: een `?task=<id>` deep-link moet altijd
// werken, want de taak bestaat dan al (hij is via de - GA - taken-sheet
// geopend, of een job_failed-taak wees hierheen). Alléén het aanvragen van een
// nieuwe verrijking ("Verrijk deze wet", zie `canEnrichLaw` hieronder) zit nog
// achter `tasks.job_review`; het beoordelen van een bestaand voorstel niet.

// Fires once the law + its first article have finished loading (whether
// that's the initial load or a tab-restore switchLaw), so it works
// regardless of how the route.query.task navigation happened to arrive.
// `reviewTaskIdParam` is itself a source: navigating from the TasksSheet
// to a wet that is ALREADY open (the target law/article is unchanged) is a
// query-only route change - `loading`/`selectedArticle` never flip, so
// without this source the watch would simply never re-fire and the review
// would never activate.
watch(
  [loading, selectedArticle, reviewTaskIdParam],
  ([isLoading, article, taskId]) => {
    if (isLoading || !article || !taskId) return;
    if (reviewAttemptedForTaskId === taskId) return;
    reviewAttemptedForTaskId = taskId;
    loadReview(taskId, currentEtag.value).then(() => {
      if (reviewProposedContent.value) applyProposedContent(reviewProposedContent.value);
    });
  },
  { immediate: true },
);

// "Verwerpen" in the review banner: resolve the task as rejected, throw
// away the seeded edit (same discard the Wijzigingenbalk offers), and
// leave review mode.
async function rejectReview() {
  await rejectReviewInternal();
  discardArticle();
  clearReviewQuery();
}

// --- "Verrijk deze wet" (request a job_review task) ---------------------
// Fire-and-forget request; the resulting job_review task shows up in the
// Taken-badge/sheet on its next poll (TasksButton/TasksSheet already poll
// via useTasks() every 30s). Use the non-polling useTaskActions() here -
// EditorView doesn't need the shared task list/badge count, and joining
// useTasks() unconditionally in setup() would start that poll for every
// editor visitor, including anonymous ones with the flag off.
const { requestEnrich } = useTaskActions();
const enrichFeedback = ref(null); // { variant, text } | null
// Flag on, an actual traject open (write access implies a traject, see
// `canEdit` above), and a law loaded - mirrors the gates other write
// actions in this view use.
const canEnrichLaw = computed(
  () => isEnabled('tasks.job_review') && canEdit.value && !!activeTrajectRef.value && !!lawId.value,
);

async function enrichLaw() {
  if (!activeTrajectRef.value || !lawId.value) return;
  try {
    const { alreadyRunning, tooMany } = await requestEnrich(activeTrajectRef.value, lawId.value);
    if (alreadyRunning) {
      enrichFeedback.value = { variant: 'alert', text: 'Er loopt al een verrijking voor deze wet.' };
    } else if (tooMany) {
      enrichFeedback.value = { variant: 'alert', text: 'Je hebt te veel verrijkingen tegelijk lopen.' };
    } else {
      enrichFeedback.value = { text: 'Verrijking gestart — je krijgt een taak zodra het resultaat klaarstaat.' };
    }
  } catch (e) {
    enrichFeedback.value = { variant: 'alert', text: 'Verrijking aanvragen mislukt.' };
  }
}
function dismissEnrichFeedback() {
  enrichFeedback.value = null;
}

// Single save handler shared by the Tekst and Machine panes. The PUT writes
// the whole law YAML, so one click persists every in-memory edit for the
// selected article regardless of which pane surfaced the button.
//
// Review-modus: Opslaan approves the task, and approval must commit the
// FULL proposal (spec §5.3/§6), not just the splice `currentLawYaml` makes
// into the single selected article - a proposal touching several articles
// would otherwise lose every article besides the one shown in the editor.
// `saveLaw` already accepts arbitrary full-law YAML text (see its PUT body
// in useLaw.js), so reusing it with `reviewProposedContent` instead of
// `currentLawYaml` is the whole fix; no second save path is introduced.
async function handleLawSave() {
  const lawYaml = reviewActive.value ? reviewProposedContent.value : currentLawYaml.value;
  if (!lawYaml) return;
  // Snapshot the law id before the await. saveLaw itself guards its own
  // reactive writes with the same check, but the post-save cleanup below
  // runs in the EditorApp scope and would happily overwrite the new law's
  // in-progress edits with its pristine article data if the user switched
  // laws mid-flight.
  const savedLawId = lawId.value;
  // In review-modus the saved YAML is the full proposal, not a splice of
  // the visible pane's edits - always treat it as touching text (notes
  // re-anchor safely; skipping would risk leaving a note's positions stale
  // against text the proposal actually changed elsewhere in the law).
  lastSaveTouchedText.value = reviewActive.value ? true : isArticleTextDirty.value;
  lastSaveTouchedMachine.value = reviewActive.value ? true : isMachineReadableDirty.value;
  try {
    await saveLaw(lawYaml);
    if (lawId.value !== savedLawId) return; // law switched mid-PUT
    // points at the re-parsed article. The `watch(selectedArticle)` above
    // fires on the next microtask - leaving a window where the dirty
    // computeds still see the pre-save values and the save button stays
    // enabled, enabling a double-save click. Reset local state explicitly
    // from the freshly-parsed article so both dirty flags clear
    // synchronously with the save.
    const fresh = selectedArticle.value;
    const freshMr = fresh?.machine_readable ?? null;
    machineReadable.value = freshMr ? structuredClone(freshMr) : null;
    yamlSource.value = freshMr ? yaml.dump(freshMr, dumpOpts) : '';
    editedText.value = fresh?.text ?? '';
    // The article text changed, so notes must re-resolve against the saved copy:
    // their quote-based selectors re-anchor via the engine, so a note whose
    // quoted text merely moved comes back at its new place, while one whose own
    // text was edited orphans (surfaced in the issues list) rather than silently
    // re-attaching. Fire-and-forget.
    if (lastSaveTouchedText.value) void reloadNotes();
    // Successful save - the dialog flags drop back to false.
    lastSaveTouchedText.value = false;
    lastSaveTouchedMachine.value = false;
    // Review-modus: a successful save IS the approval (spec §5.3 - save
    // first, then resolve). Runs after the dirty-state reset above so the
    // Wijzigingenbalk has already cleared before the task disappears.
    if (reviewActive.value) {
      await approveAfterSave();
      clearReviewQuery();
    }
  } catch (e) {
    // saveError is surfaced via lawSaveError; log for dev visibility.
    console.warn('saveLaw failed:', e);
  }
}

// Whole-law save failures surface as a single modal over the editor (not an
// inline dialog buried in one pane). lawSaveError drives it: a new failure
// (a fresh Error) re-opens it via the watch, a successful save (null) closes it.
const saveErrorModalEl = ref(null);
watch(lawSaveError, (err) => {
  const el = saveErrorModalEl.value;
  if (!el) return;
  if (err && typeof el.show === 'function') el.show();
  else if (!err && typeof el.hide === 'function') el.hide();
});
function dismissSaveError() {
  saveErrorModalEl.value?.hide?.();
}

// Alias kept to minimise template churn; both panes ultimately call the
// same whole-law save.
const handleMachineReadableSave = handleLawSave;

// --- Wijzigingenbalk (article-level pending changes) -----------------------
// Tekst + Machine edits share one whole-law save, so "dirty" is article-wide.
// The AppShell renders the bar; the editor publishes the state and the
// Opslaan / Wijzigingen-ongedaan / undo / redo actions via useAppChrome.
const articleDirty = computed(
  () => isArticleTextDirty.value || isMachineReadableDirty.value,
);

// Throw away every in-memory edit for the selected article - the same reset
// the post-save cleanup performs, minus the PUT.
function discardArticle() {
  const fresh = selectedArticle.value;
  if (!fresh) return;
  const freshMr = fresh.machine_readable ?? null;
  machineReadable.value = freshMr ? structuredClone(freshMr) : null;
  yamlSource.value = freshMr ? yaml.dump(freshMr, dumpOpts) : '';
  editedText.value = fresh.text ?? '';
  // editedText flows into each Tekst pane's Tiptap content via its model watch
  // on the next tick; clear that history afterwards so Ctrl+Z can't step back
  // into the discarded edits and re-dirty the article.
  nextTick(() => {
    for (const refs of Object.values(textEditorRefs)) refs?.clearHistory?.();
  });
}

// Undo/redo route to the last-focused Tekst editor (falling back to the first
// mounted one). Tiptap owns the history; the Machine/YAML panes have none, so
// the buttons simply stay disabled when no text editor can undo.
const activeTextEditorIdx = ref(null);
function onTextEditorFocus(idx) { activeTextEditorIdx.value = idx; }
const targetTextEditor = computed(() => {
  const active = activeTextEditorIdx.value != null
    ? textEditorRefs[activeTextEditorIdx.value]
    : null;
  if (active) return active;
  const firstKey = Object.keys(textEditorRefs)[0];
  return firstKey != null ? textEditorRefs[firstKey] : null;
});
const canUndoText = computed(() => targetTextEditor.value?.canUndo ?? false);
const canRedoText = computed(() => targetTextEditor.value?.canRedo ?? false);
function undoText() { targetTextEditor.value?.undo?.(); }
function redoText() { targetTextEditor.value?.redo?.(); }

// Stable actions registered once; the reactive state re-publishes via the
// watchEffect so the bar re-renders as dirty/saving/undo-availability shift.
registerEditorActions({
  save: handleLawSave,
  discard: discardArticle,
  undo: undoText,
  redo: redoText,
});
watchEffect(() => {
  setEditorChanges({
    dirty: articleDirty.value,
    saving: lawSaving.value,
    canUndo: canUndoText.value,
    canRedo: canRedoText.value,
  });
});

// Reflect navigation depth + unsaved state in the document title:
//   "• Editor: Art. 5 · Wet op de zorgtoeslag · 15 juni test · RegelRecht"
// A leading dot flags unsaved changes - it stays visible even when the tab
// title is truncated to its start. Then the mode prefix, then most-specific to
// least-specific (article, law, traject) with the brand last. Lives here (after
// articleDirty) and is always set - a static router.afterEach fallback used to
// race this effect on tab/article switches.
watchEffect(() => {
  const detail = [];
  if (selectedArticle.value) detail.push(`Art. ${selectedArticle.value.number}`);
  if (lawName.value) detail.push(lawName.value);
  if (activeTraject.value?.name) detail.push(activeTraject.value.name);
  const base = detail.length > 0
    ? `Editor: ${detail.join(' · ')} · RegelRecht`
    : 'Editor · RegelRecht';
  document.title = articleDirty.value ? `• ${base}` : base;
});

function onYamlInput(event) {
  // nldd-code-editor dispatches a CustomEvent with the new value in
  // event.detail.value (see the design-system 0.8.41 component). The
  // host's `value` property is updated before dispatch so
  // event.target.value would also work, but reading from detail keeps
  // the contract explicit and matches how the storybook docs the API.
  // `??` skips only null/undefined - a deliberate empty string passes
  // through as a valid "user cleared the editor" input.
  const text = event.detail?.value ?? event.target?.value;
  if (text == null) {
    // Structurally broken event (no detail, no value on target). Don't
    // touch yamlSource - silently overwriting with the previous value
    // would swallow keystrokes a user can see in the textarea but
    // never sees committed to the reactive source. Surface it loudly
    // instead so the regression is obvious in dev console.
    console.warn('onYamlInput: unexpected event shape, ignoring', event);
    return;
  }
  yamlSource.value = text;
  try {
    const parsed = yaml.load(text);
    machineReadable.value = parsed != null && typeof parsed === 'object' ? parsed : null;
    parseError.value = null;
  } catch (e) {
    parseError.value = e.message;
  }
}

function handleSave({ section, key, newKey, index, data }) {
  // JSON round-trip clone: Vue reactive proxies aren't structuredClone-able
  // in all envs. Safe because law YAML is JSON-plain - but Date/undefined
  // are NOT preserved, which only matters if a future field becomes
  // date-typed (use an explicit serialiser then).
  const mr = machineReadable.value
    ? JSON.parse(JSON.stringify(machineReadable.value))
    : {};

  if (!mr.definitions) mr.definitions = {};
  if (!mr.execution) mr.execution = {};
  if (!mr.execution.parameters) mr.execution.parameters = [];
  if (!mr.execution.input) mr.execution.input = [];
  if (!mr.execution.output) mr.execution.output = [];

  if (section === 'definition') {
    if (newKey && newKey !== key) delete mr.definitions[key];
    mr.definitions[newKey || key] = data;
  } else if (section === 'add-definition') {
    mr.definitions[key] = data;
  } else if (section === 'parameter') {
    mr.execution.parameters[index] = data;
  } else if (section === 'add-parameter') {
    mr.execution.parameters.push(data);
  } else if (section === 'input') {
    mr.execution.input[index] = data;
  } else if (section === 'add-input') {
    mr.execution.input.push(data);
  } else if (section === 'output') {
    mr.execution.output[index] = data;
  } else if (section === 'add-output') {
    mr.execution.output.push(data);
  } else if (section === 'produces') {
    if (!mr.execution.produces) mr.execution.produces = {};
    if (data == null) delete mr.execution.produces[key];
    else mr.execution.produces[key] = data;
    // Drop the container once every field is cleared, otherwise the YAML
    // dump emits a bare `produces: {}` which the schema rejects.
    if (Object.keys(mr.execution.produces).length === 0) {
      delete mr.execution.produces;
    }
  }

  machineReadable.value = mr;
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}

// Delete an item from the machine_readable. Mirrors handleSave's section
// dispatch but removes the entry instead of replacing it. Definitions are
// keyed by name; parameters / inputs / outputs / actions are keyed by
// array index. Out-of-range indices and missing keys are no-ops so a
// stale event from the UI can never crash.
function handleDelete({ section, key, index }) {
  // JSON clone - see handleSave: Date/undefined not preserved (fine for
  // JSON-plain law data).
  const mr = machineReadable.value
    ? JSON.parse(JSON.stringify(machineReadable.value))
    : null;
  if (!mr) return;

  if (section === 'definition') {
    if (mr.definitions && key != null && key in mr.definitions) {
      delete mr.definitions[key];
    }
  } else if (section === 'parameter') {
    if (mr.execution?.parameters && index >= 0 && index < mr.execution.parameters.length) {
      mr.execution.parameters.splice(index, 1);
    }
  } else if (section === 'input') {
    if (mr.execution?.input && index >= 0 && index < mr.execution.input.length) {
      mr.execution.input.splice(index, 1);
    }
  } else if (section === 'output') {
    if (mr.execution?.output && index >= 0 && index < mr.execution.output.length) {
      mr.execution.output.splice(index, 1);
    }
  } else if (section === 'action') {
    if (mr.execution?.actions && index >= 0 && index < mr.execution.actions.length) {
      mr.execution.actions.splice(index, 1);
    }
  }

  machineReadable.value = mr;
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}

// Initialize empty machine_readable scaffold
function handleInitMr() {
  machineReadable.value = {
    definitions: {},
    execution: {
      parameters: [],
      input: [],
      output: [],
      actions: [],
    },
  };
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}

// Add a new action and open ActionSheet
let actionSnapshot = null;

function handleAddAction() {
  // Snapshot BEFORE any mutations so cancel restores the exact original state
  actionSnapshot = JSON.stringify(machineReadable.value);
  const mr = machineReadable.value || {};
  if (!mr.execution) mr.execution = {};
  if (!mr.execution.actions) mr.execution.actions = [];
  // Seed the new action with an EQUALS stub instead of an empty literal so
  // OperationSettings has an operation tree to render and the user can
  // immediately reach the type dropdown to switch to AGE / AND / etc.
  // The findIncompleteOperation guard rejects unfilled stubs on save, so
  // a half-configured action still can't be persisted.
  const newAction = {
    output: '',
    value: { operation: 'EQUALS', subject: '', value: '' },
  };
  mr.execution.actions.push(newAction);
  machineReadable.value = { ...mr };
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
  activeActionIsNew.value = true;
  activeAction.value = newAction;
}

function handleOpenAction(action) {
  actionSnapshot = JSON.stringify(machineReadable.value);
  activeActionIsNew.value = false;
  activeAction.value = action;
  // Clear any stale parse error from a previous failed save
  parseError.value = null;
}

// Restore model from snapshot when ActionSheet is cancelled
function handleActionClose() {
  if (actionSnapshot) {
    machineReadable.value = JSON.parse(actionSnapshot);
    yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
    actionSnapshot = null;
  }
  activeAction.value = null;
  // Clear any stale parse error from a failed save attempt
  parseError.value = null;
}

const COMPARISON_OPS_SET = new Set([
  'EQUALS', 'NOT_EQUALS', 'GREATER_THAN', 'GREATER_THAN_OR_EQUAL',
  'LESS_THAN', 'LESS_THAN_OR_EQUAL', 'NOT_NULL', 'IN', 'NOT_IN',
]);

// Walk a value tree and report the first incomplete operation (e.g. a stub
// `{ operation: 'ADD', values: [] }` that the user inserted via "Voeg operatie
// toe" but never filled in). Returns null when the tree is structurally valid.
function findIncompleteOperation(value) {
  if (value == null || typeof value !== 'object') return null;
  if (!value.operation) return null;
  const op = value.operation;
  // Arithmetic / logical ops need a non-empty values or conditions array
  if (Array.isArray(value.values) && value.values.length === 0) return op;
  if (Array.isArray(value.conditions) && value.conditions.length === 0) return op;
  // IF/SWITCH need at least one case
  if ((op === 'IF' || op === 'SWITCH') && (!Array.isArray(value.cases) || value.cases.length === 0)) return op;
  // Comparison ops need a non-empty subject (and value, except for NOT_NULL).
  // changeOperationType / addNestedOperation seed these as empty strings, so
  // we must reject the stub before persisting. IN/NOT_IN accept either a
  // variable reference (e.g. "$list") or a literal non-empty array; both
  // are non-empty by the same value !== '' / array.length > 0 check.
  if (COMPARISON_OPS_SET.has(op)) {
    if ((value.subject ?? '') === '') return op;
    if (op !== 'NOT_NULL') {
      const v = value.value;
      if (v == null || v === '') return op;
      if (Array.isArray(v) && v.length === 0) return op;
    }
  }
  // NOT wraps a single value/operation; reject the empty-string stub created
  // when transitioning from arithmetic ops via changeOperationType.
  if (op === 'NOT' && (value.value ?? '') === '') return op;
  // AGE has two structural slots - both must be filled. Empty strings are
  // the seed values from changeOperationType('AGE'); reject them so the
  // user can't save a stub.
  if (op === 'AGE') {
    if ((value.date_of_birth ?? '') === '') return op;
    if ((value.reference_date ?? '') === '') return op;
  }
  // Recurse into structural slots
  for (const child of [value.subject, value.value, value.default, value.date_of_birth, value.reference_date]) {
    const inner = findIncompleteOperation(child);
    if (inner) return inner;
  }
  if (Array.isArray(value.values)) {
    for (const v of value.values) {
      const inner = findIncompleteOperation(v);
      if (inner) return inner;
    }
  }
  if (Array.isArray(value.conditions)) {
    for (const c of value.conditions) {
      const inner = findIncompleteOperation(c);
      if (inner) return inner;
    }
  }
  if (Array.isArray(value.cases)) {
    for (const c of value.cases) {
      const inner = findIncompleteOperation(c?.when) || findIncompleteOperation(c?.then);
      if (inner) return inner;
    }
  }
  return null;
}

// Sync YAML when ActionSheet saves (mutations happened in-place)
async function handleActionSave() {
  const action = activeAction.value;
  if (action) {
    // Output is required by the schema and the engine cannot load a law
    // with an action that has an empty output name.
    if (action.output == null || String(action.output).trim() === '') {
      parseError.value = 'Output mag niet leeg zijn';
      return;
    }
    // Reject incomplete nested operations (e.g. ADD with empty values[]) that
    // the user inserted via "Voeg operatie toe" but never filled in.
    // Note: a literal empty-string `value` is permitted at this layer - the
    // schema validator on save handles type-specific validation; rejecting it
    // here would block the legitimate "set output now, fill value via YAML
    // pane later" workflow used by the test suite and the editor's manual
    // YAML escape hatch.
    const incomplete = findIncompleteOperation(action.value);
    if (incomplete) {
      parseError.value = `Operatie '${incomplete}' is nog niet ingevuld`;
      return;
    }
  }
  // Commit the in-place mutations, then actually persist the law. The
  // sheet's "Opslaan" is a real save, so the Machine pane won't show a
  // separate dirty/save affordance for sheet edits. On a failed PUT the
  // edits stay in the model and the Machine pane's normal dirty/save
  // affordance is the fallback - no data loss either way.
  // JSON clone - see handleSave: reactive proxy not structuredClone-able;
  // Date/undefined not preserved (fine for JSON-plain law YAML).
  machineReadable.value = JSON.parse(JSON.stringify(machineReadable.value));
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
  // Close the sheet unconditionally: the mutations are already committed
  // above, so even if handleLawSave() throws the sheet must not stay open
  // showing a now-clean (isDirty === false) state. A failed PUT falls back
  // to the Machine pane's normal dirty/save affordance - no data loss.
  try {
    await handleLawSave();
  } finally {
    actionSnapshot = null;
    activeAction.value = null;
  }
}

</script>

<template>
        <!-- The URL names a traject the user has no membership for: it was
             deleted, never existed, or access was revoked. We do not
             distinguish these (no leak of traject existence). Takes
             precedence over every editor state below, including the law-404
             branch, so a missing traject never shows "<law> is niet
             beschikbaar in dit traject". -->
        <nldd-page v-if="trajectMissing">
          <nldd-simple-section width="full">
            <nldd-inline-dialog
              variant="alert"
              text="Dit traject bestaat niet of je hebt geen toegang."
              supporting-text="Ga terug naar het overzicht om een traject te kiezen."
            >
              <nldd-button slot="actions" variant="primary" text="Naar overzicht" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- Empty state: no tabs open. The CTA points back to the library
             since that's the only way to create new tabs; mention the tab
             bar too because closed tabs may still be visible alongside this
             empty state on the next pane. -->
        <nldd-page v-else-if="!activeTab">
          <nldd-simple-section width="full">
            <nldd-inline-dialog text="Open een artikel vanuit de tabbalk of Home om te bewerken.">
              <nldd-button slot="actions" variant="secondary" text="Naar Home" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- Loading takes precedence over `error` to avoid flashing a stale error during a refetch. -->
        <nldd-page v-else-if="loading">
          <nldd-simple-section width="full">
            <nldd-activity-indicator timing="instant" text="Artikel laden" show-text></nldd-activity-indicator>
          </nldd-simple-section>
        </nldd-page>

        <!-- Error state - mirrors the library's law-load failure pattern.
             404s typically mean "the law isn't part of the active traject"
             (e.g. after a traject switch); we surface a traject-specific
             message and a quick "Naar bibliotheek" exit. Other failures
             keep the generic copy + retry. -->
        <nldd-page v-else-if="error">
          <nldd-simple-section width="full">
            <nldd-inline-dialog
              v-if="lawErrorIs404"
              variant="alert"
              :text="`${failedLawName} is niet beschikbaar in dit traject`"
              supporting-text="Wissel van traject via het menu rechtsboven of ga terug naar het overzicht."
            >
              <nldd-button slot="actions" variant="primary" text="Naar Home" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
              <nldd-button slot="actions" variant="secondary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
            </nldd-inline-dialog>
            <nldd-inline-dialog
              v-else
              variant="alert"
              :text="`${failedLawName} is niet geladen`"
              supporting-text="De gegevens konden niet worden opgehaald."
            >
              <nldd-button slot="actions" variant="primary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
              <nldd-button slot="actions" variant="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- All editor flags off - paneViews is empty so the
             side-by-side view would render zero pane slots. Surface an
             explicit empty-state with a CTA to the settings menu so
             the user understands the editor isn't broken. -->
        <nldd-page v-else-if="paneViews.length === 0">
          <nldd-simple-section width="full">
            <nldd-inline-dialog text="Geen editors actief. Schakel ten minste één editor in via Instellingen."></nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- One pane per entry in `paneViews`, wrapped in a template so the
             review/enrich banners can sit above it as siblings within this
             same v-else branch. Each pane independently picks its view via
             the dropdown in its header. The split-view auto-hides panes
             from the right when the viewport is too narrow. Hidden panes
             stay in the DOM so state is preserved when the viewport
             widens. -->
        <template v-else>
          <!-- Review-modus (job_review-taak) + "Verrijk deze wet"-feedback,
               each state's own ndd-page/ndd-simple-section (matching the
               sibling states above) rather than bare dialogs floating in
               the split-view's slot. -->
          <nldd-page v-if="reviewActive || reviewLoadError || enrichFeedback">
            <nldd-simple-section width="full">
              <!-- Shown while a task's proposal is seeded as an unsaved
                   edit, or when loading the task failed (loadError, no
                   content applied then). -->
              <nldd-inline-dialog
                v-if="reviewActive || reviewLoadError"
                :variant="reviewLoadError || reviewStale || (reviewActive && !reviewSeeded) ? 'alert' : undefined"
                text="Voorstel uit verrijking"
                :supporting-text="reviewSupportingText"
              >
                <!-- Wijzigingenbalk only appears when a pane is dirty, which
                     `!reviewSeeded` never is (nothing was seeded into the
                     panes) - give the banner its own primary action so
                     approving a proposal that touches nothing visible here
                     is still reachable. -->
                <nldd-button
                  v-if="reviewActive && !reviewSeeded"
                  slot="actions"
                  variant="primary"
                  text="Voorstel opslaan en goedkeuren"
                  :loading="lawSaving || undefined"
                  :disabled="lawSaving || undefined"
                  @click="handleLawSave"
                ></nldd-button>
                <nldd-button
                  v-if="reviewActive"
                  slot="actions"
                  variant="secondary"
                  text="Verwerpen"
                  @click="rejectReview"
                ></nldd-button>
              </nldd-inline-dialog>

              <!-- Feedback from "Verrijk deze wet" (see the pane toolbar below). -->
              <nldd-inline-dialog
                v-if="enrichFeedback"
                :variant="enrichFeedback.variant"
                :text="enrichFeedback.text"
              >
                <nldd-button slot="actions" text="Sluiten" @click="dismissEnrichFeedback"></nldd-button>
              </nldd-inline-dialog>
            </nldd-simple-section>
          </nldd-page>

        <nldd-side-by-side-split-view :panes="String(paneViews.length)">
          <!-- Compound key: when a flag flip shifts which view sits at a
               given index, Vue would otherwise patch the existing pane in
               place - leaking ScenarioBuilder form state and engine
               results into a different view. Re-keying on the view id
               forces an unmount + remount on identity change, at the
               (acceptable) cost of losing pane scroll position. -->
          <nldd-split-view-pane
            v-for="(view, idx) in paneViews"
            :key="`${view}-${idx}`"
            :slot="`pane-${idx + 1}`"
          >
            <nldd-page
              sticky-header
              :background="view === 'scenario' ? 'base' : undefined"
            >
              <nldd-container slot="header" padding="8" padding-bottom="0">
                <nldd-toolbar size="md" label="Paneelacties">
                  <!-- Weergave-keuze (alle panes). Hoogste prioriteit zodat
                       deze als laatste naar het overflow-menu verhuist. -->
                  <nldd-toolbar-item slot="start" label="Weergave" :priority="4">
                    <nldd-button
                      :id="`pane-view-btn-${idx}`"
                      size="md"
                      expandable
                      :text="viewLabel(view)"
                      :popovertarget="`pane-view-menu-${idx}`"
                    ></nldd-button>
                    <nldd-menu :id="`pane-view-menu-${idx}`" :anchor="`pane-view-btn-${idx}`">
                      <nldd-menu-item
                        v-for="opt in availableViews"
                        :key="opt.id"
                        type="radio"
                        :selected="view === opt.id || undefined"
                        :text="opt.label"
                        @select="setPaneView(idx, opt.id)"
                      ></nldd-menu-item>
                      <nldd-menu-divider></nldd-menu-divider>
                      <nldd-menu-item
                        text="Verplaats naar links"
                        icon="arrow-left"
                        :disabled="idx === 0 || undefined"
                        @select="movePane(idx, 'left')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats naar rechts"
                        icon="arrow-right"
                        :disabled="idx === paneViews.length - 1 || undefined"
                        @select="movePane(idx, 'right')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats uiterst links"
                        icon="arrow-left-to-line"
                        :disabled="idx === 0 || undefined"
                        @select="movePane(idx, 'start')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats uiterst rechts"
                        icon="arrow-right-to-line"
                        :disabled="idx === paneViews.length - 1 || undefined"
                        @select="movePane(idx, 'end')"
                      ></nldd-menu-item>
                    </nldd-menu>
                    <nldd-menu-group slot="overflow" text="Weergave">
                      <nldd-menu-item
                        v-for="opt in availableViews"
                        :key="`ovf-view-${opt.id}`"
                        type="radio"
                        :selected="view === opt.id || undefined"
                        :text="opt.label"
                        @select="setPaneView(idx, opt.id)"
                      ></nldd-menu-item>
                      <nldd-menu-divider></nldd-menu-divider>
                      <nldd-menu-item
                        text="Verplaats naar links"
                        icon="arrow-left"
                        :disabled="idx === 0 || undefined"
                        @select="movePane(idx, 'left')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats naar rechts"
                        icon="arrow-right"
                        :disabled="idx === paneViews.length - 1 || undefined"
                        @select="movePane(idx, 'right')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats uiterst links"
                        icon="arrow-left-to-line"
                        :disabled="idx === 0 || undefined"
                        @select="movePane(idx, 'start')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats uiterst rechts"
                        icon="arrow-right-to-line"
                        :disabled="idx === paneViews.length - 1 || undefined"
                        @select="movePane(idx, 'end')"
                      ></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- Vet/Schuin - checkbox segmented control (beide kunnen
                       tegelijk actief zijn). Bron van waarheid: de Tiptap-
                       editor; de control reflecteert de selectie. -->
                  <nldd-toolbar-item
                    v-if="view === 'text' && selectedArticle && textEditorRefs[idx]"
                    slot="end"
                    label="Tekststijl"
                    :priority="2"
                  >
                    <nldd-segmented-control
                      type="checkbox"
                      variant="icon"
                      size="md"
                      accessible-label="Tekststijl"
                      :disabled="!canEditArticleText || undefined"
                      :values.prop="boldItalicValues(idx)"
                      @item-change="onInlineFormatChange(idx, $event)"
                    >
                      <nldd-segmented-control-item value="bold" icon="bold" text="Vet"></nldd-segmented-control-item>
                      <nldd-segmented-control-item value="italic" icon="italic" text="Schuin"></nldd-segmented-control-item>
                    </nldd-segmented-control>
                    <nldd-menu-group slot="overflow" text="Tekststijl">
                      <nldd-menu-item
                        type="checkbox"
                        icon="bold"
                        text="Vet"
                        :selected="textEditorRefs[idx].activeFormats.bold || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="textEditorRefs[idx].toggleBold()"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        type="checkbox"
                        icon="italic"
                        text="Schuin"
                        :selected="textEditorRefs[idx].activeFormats.italic || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="textEditorRefs[idx].toggleItalic()"
                      ></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- Lijsttype - radio segmented control: geen / opsomming /
                       genummerd. "Geen" (minus-small) heft de actieve lijst op. -->
                  <nldd-toolbar-item
                    v-if="view === 'text' && selectedArticle && textEditorRefs[idx]"
                    slot="end"
                    label="Lijst"
                    :priority="1"
                  >
                    <nldd-segmented-control
                      type="radio"
                      variant="icon"
                      size="md"
                      accessible-label="Lijst"
                      :disabled="!canEditArticleText || undefined"
                      :value="listValue(idx)"
                      @change="onListChange(idx, $event)"
                    >
                      <nldd-segmented-control-item value="none" icon="minus-small" text="Geen lijst"></nldd-segmented-control-item>
                      <nldd-segmented-control-item value="bullet" icon="bullet-list" text="Opsomming"></nldd-segmented-control-item>
                      <nldd-segmented-control-item value="ordered" icon="numbered-list" text="Genummerde lijst"></nldd-segmented-control-item>
                    </nldd-segmented-control>
                    <nldd-menu-group slot="overflow" text="Lijst">
                      <nldd-menu-item
                        type="radio"
                        icon="minus-small"
                        text="Geen lijst"
                        :selected="listValue(idx) === 'none' || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="setList(idx, 'none')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        type="radio"
                        icon="bullet-list"
                        text="Opsomming"
                        :selected="listValue(idx) === 'bullet' || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="setList(idx, 'bullet')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        type="radio"
                        icon="numbered-list"
                        text="Genummerde lijst"
                        :selected="listValue(idx) === 'ordered' || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="setList(idx, 'ordered')"
                      ></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- Inspringen - vergroot/verklein de inspringing van de
                       geselecteerde regels. Laagste prioriteit (0), dus deze
                       verdwijnt als eerste naar de overflow. -->
                  <nldd-toolbar-item
                    v-if="view === 'text' && selectedArticle && textEditorRefs[idx]"
                    slot="end"
                    label="Inspringen"
                    :priority="0"
                  >
                    <!-- Grouped as a button-bar: the bar draws the divider and
                         propagates size/variant to its children. Individual
                         disabled stays per-button (canIndent vs canOutdent) since
                         the bar itself is never disabled. -->
                    <nldd-button-bar size="md">
                      <nldd-icon-button
                        icon="indent-increase"
                        text="Inspringen vergroten"
                        :disabled="!canEditArticleText || !textEditorRefs[idx].activeFormats.canIndent || undefined"
                        @mousedown.prevent
                        @click="textEditorRefs[idx].indent()"
                      ></nldd-icon-button>
                      <nldd-button-bar-divider></nldd-button-bar-divider>
                      <nldd-icon-button
                        icon="indent-decrease"
                        text="Inspringen verkleinen"
                        :disabled="!canEditArticleText || !textEditorRefs[idx].activeFormats.canOutdent || undefined"
                        @mousedown.prevent
                        @click="textEditorRefs[idx].outdent()"
                      ></nldd-icon-button>
                    </nldd-button-bar>
                    <nldd-menu-group slot="overflow" text="Inspringen">
                      <nldd-menu-item
                        icon="indent-increase"
                        text="Inspringen vergroten"
                        :disabled="!canEditArticleText || !textEditorRefs[idx].activeFormats.canIndent || undefined"
                        @select="textEditorRefs[idx].indent()"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        icon="indent-decrease"
                        text="Inspringen verkleinen"
                        :disabled="!canEditArticleText || !textEditorRefs[idx].activeFormats.canOutdent || undefined"
                        @select="textEditorRefs[idx].outdent()"
                      ></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- Notitie toevoegen - annotatie op de huidige selectie.
                       Prioriteit tussen de mode-switcher (4) en de opmaak-
                       controls (Tekststijl 2, Lijst 1) in, zodat deze primaire
                       actie pas na de opmaak naar de overflow verdwijnt. -->
                  <nldd-toolbar-item
                    v-if="view === 'text' && selectedArticle && textEditorRefs[idx] && canCreateNotes"
                    slot="end"
                    label="Notitie"
                    :priority="3"
                  >
                    <nldd-icon-button
                      icon="comment"
                      text="Notitie toevoegen"
                      variant="secondary"
                      size="md"
                      :disabled="textEditorRefs[idx].selectionEmpty || undefined"
                      @mousedown.prevent
                      @click="startNoteFromSelection(idx)"
                    ></nldd-icon-button>
                    <!-- No :disabled here: the toolbar clones overflow items, so
                         a live-changing disabled (selectionEmpty) would freeze at
                         clone time and stick. startNoteFromSelection no-ops on an
                         empty selection instead. -->
                    <nldd-menu-item
                      slot="overflow"
                      icon="comment"
                      text="Notitie toevoegen"
                      @select="startNoteFromSelection(idx)"
                    ></nldd-menu-item>
                  </nldd-toolbar-item>
                  <!-- Notities downloaden - expandable icon-button (area end) met
                       een menu: dit artikel, of de hele wet, als YAML. -->
                  <nldd-toolbar-item
                    v-if="view === 'notes' && selectedArticle && notesForArticle.length > 0"
                    slot="end"
                    label="Notities downloaden"
                    :priority="3"
                  >
                    <nldd-icon-button
                      :id="`notes-export-btn-${idx}`"
                      icon="download"
                      text="Notities downloaden"
                      variant="secondary"
                      size="md"
                      expandable
                      :popovertarget="`notes-export-menu-${idx}`"
                    ></nldd-icon-button>
                    <nldd-menu :id="`notes-export-menu-${idx}`" :anchor="`notes-export-btn-${idx}`">
                      <nldd-menu-item icon="document" text="Artikel-notities als YAML" @select="exportArticleNotes"></nldd-menu-item>
                      <nldd-menu-item icon="document" text="Wet-notities als YAML" @select="exportNotes"></nldd-menu-item>
                    </nldd-menu>
                    <nldd-menu-group slot="overflow" text="Notities downloaden">
                      <nldd-menu-item icon="document" text="Artikel-notities als YAML" @select="exportArticleNotes"></nldd-menu-item>
                      <nldd-menu-item icon="document" text="Wet-notities als YAML" @select="exportNotes"></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- YAML parse-status (Machine-readable pane). -->
                  <nldd-toolbar-item v-if="view === 'yaml' && parseError" slot="end" label="YAML">
                    <span class="editor-parse-error">YAML parse error</span>
                    <nldd-menu-item
                      slot="overflow"
                      text="YAML parse error"
                      disabled
                    ></nldd-menu-item>
                  </nldd-toolbar-item>
                  <!-- Wet-acties: only on the first pane - the action applies to
                       the whole law, not this one pane, so showing it in every
                       pane's own toolbar would just duplicate it. -->
                  <nldd-toolbar-item
                    v-if="idx === 0 && canEnrichLaw"
                    slot="end"
                    label="Wet acties"
                    :priority="1"
                  >
                    <nldd-icon-button
                      id="law-actions-btn"
                      icon="ai"
                      text="Wet acties"
                      variant="secondary"
                      size="md"
                      expandable
                      popovertarget="law-actions-menu"
                    ></nldd-icon-button>
                    <nldd-menu id="law-actions-menu" anchor="law-actions-btn">
                      <nldd-menu-item icon="ai" text="Verrijk deze wet" @select="enrichLaw"></nldd-menu-item>
                    </nldd-menu>
                    <nldd-menu-group slot="overflow" text="Wet acties">
                      <nldd-menu-item icon="ai" text="Verrijk deze wet" @select="enrichLaw"></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                </nldd-toolbar>
              </nldd-container>

              <!-- Tekst - WYSIWYG editor when the user can edit, otherwise the
                   read-only ArticleText display. The format controls in the
                   header toolbar guard on `textEditorRefs[idx]`, which only the
                   WYSIWYG component populates, so they auto-hide when read-only. -->
              <nldd-simple-section v-if="view === 'text'" width="full">
                <ArticleTextEditor
                  v-if="canEditArticleText"
                  :ref="setTextEditorRef(idx)"
                  :article="selectedArticle"
                  :editable="canEditArticleText"
                  :model-value="editedText"
                  :annotations="editorAnnotations"
                  @update:model-value="editedText = $event"
                  @focus="onTextEditorFocus(idx)"
                  @annotation-click="onEditorAnnotationClick"
                />
                <ArticleText v-else :article="selectedArticle" />

                <!-- The note editor and list live in one right sheet (the badge
                     look-ahead popover, below, is the lightweight step before it).
                     The badge list is the root view; editing/adding pushes an
                     editor view (with a back button) or, from the notities pane,
                     opens it as the root (with a close button). -->
                <nldd-sheet
                  :ref="(el) => (noteSheetEl = el)"
                  placement="right"
                  width="480px"
                  accessible-label="Notities"
                  @close="onNoteSheetClose"
                >
                  <!-- List view: the referenced text, a card per note, then add.
                       A sheet always hosts an nldd-page; the title bar is its
                       header (sticky) and carries the close button. -->
                  <template v-if="noteSheetView === 'list'">
                    <nldd-page sticky-header>
                      <nldd-top-title-bar
                        slot="header"
                        text="Notities"
                        dismiss-text="Sluit"
                        @dismiss="noteSheetEl?.hide?.()"
                      ></nldd-top-title-bar>
                      <nldd-container padding="16" data-testid="note-detail">
                        <QuotedFragment :fragment="activeGroup" />
                        <nldd-spacer v-if="activeGroup && activeGroup.quote" size="12"></nldd-spacer>
                        <nldd-collection layout="stack" gap="12px">
                          <nldd-card v-for="(note, i) in activeNotes" :key="i">
                            <nldd-container padding="10">
                              <NoteCard
                                :note="note"
                                :can-edit="canEdit"
                                :saving="savingNotes"
                                @edit="startEditNote(groupForNote(note), note, true)"
                                @share="askPublishNote(note)"
                                @delete="askDeleteNote(note)"
                              />
                            </nldd-container>
                          </nldd-card>
                        </nldd-collection>
                        <template v-if="canCreateNotes && activeGroup && activeGroup.quote">
                          <nldd-spacer size="12"></nldd-spacer>
                          <nldd-button
                            variant="secondary"
                            size="md"
                            width="full"
                            start-icon="add"
                            text="Notitie toevoegen"
                            @click="startNoteForGroup(activeGroup, true)"
                          ></nldd-button>
                        </template>
                      </nldd-container>
                    </nldd-page>
                  </template>

                  <!-- Editor view: back button when it came from the list, plus
                       the close button on the right in every case. -->
                  <template v-else>
                    <nldd-page sticky-header>
                      <nldd-top-title-bar
                        slot="header"
                        text="Notitie"
                        :back-text="noteEditFromList ? 'Notities' : ''"
                        dismiss-text="Annuleer"
                        @back="noteEditorBack"
                        @dismiss="noteSheetEl?.hide?.()"
                      ></nldd-top-title-bar>
                      <NoteCreator
                        v-if="noteCreator.open"
                        :range="noteCreator.range"
                        :raw-text="editedText"
                        :law-id="lawId"
                        :article="selectedArticle"
                        :engine="noteEngine"
                        :traject-ref="activeTrajectRef || ''"
                        :initial-note="noteCreator.initialNote"
                        @create="onNoteCreated"
                        @cancel="noteEditCancel"
                      />
                    </nldd-page>
                  </template>
                </nldd-sheet>

                <!-- Look-ahead popover on a badge click: a flat preview of the
                     comments (no referenced text) with a CTA into the sheet. -->
                <nldd-popover
                  :ref="(el) => (annotationPopEl = el)"
                  accessible-label="Notities"
                  placement="bottom-start"
                >
                  <nldd-container padding="16">
                    <template v-for="(note, i) in activeNotes" :key="i">
                      <nldd-spacer v-if="i > 0" size="16"></nldd-spacer>
                      <nldd-rich-text>
                        <p>{{ noteText(note) || '-' }}</p>
                      </nldd-rich-text>
                      <template v-if="noteAuthor(note)">
                        <nldd-spacer size="4"></nldd-spacer>
                        <nldd-byline :text="noteAuthor(note)"></nldd-byline>
                      </template>
                    </template>
                    <nldd-spacer size="16"></nldd-spacer>
                    <nldd-button
                      variant="secondary"
                      width="full"
                      text="Bijdragen of bewerken"
                      @click="openSheetFromAnnotation"
                    ></nldd-button>
                  </nldd-container>
                </nldd-popover>

                <!-- Note management + the note list live in the Notities pane. -->
              </nldd-simple-section>

              <!-- Machine readable -->
              <nldd-simple-section v-else-if="view === 'machine'" width="full">
                <MachineReadable
                  :article="editedArticle"
                  :editable="canEdit"
                  :dirty="isMachineReadableDirty"
                  :saving="lawSaving"
                  :traject-ref="activeTrajectRef"
                  @open-action="handleOpenAction"
                  @open-edit="activeEditItem = $event"
                  @init-mr="handleInitMr"
                  @add-action="handleAddAction"
                  @save="handleMachineReadableSave"
                  @patch="handleSave"
                  @delete="handleDelete"
                />
              </nldd-simple-section>

              <!-- Scenario builder -->
              <template v-else-if="view === 'scenario'">
                <nldd-simple-section v-if="engineInitError" width="full">
                  <nldd-inline-dialog
                    variant="alert"
                    text="WASM engine niet geladen"
                    :supporting-text="`${engineInitError.message} - voer 'just wasm-build' uit om de WASM module te bouwen.`"
                  ></nldd-inline-dialog>
                </nldd-simple-section>
                <ScenarioBuilder
                  v-else
                  :law-id="lawId"
                  :law-yaml="currentLawYaml"
                  :engine="getEngine()"
                  :ready="engineReady"
                  :articles="articles"
                  :traject-ref="activeTrajectRef"
                  @executed="handleScenarioExecuted"
                />
              </template>

              <!-- YAML -->
              <nldd-simple-section v-else-if="view === 'yaml'" width="full">
                <nldd-inline-dialog
                  v-if="!hasMachineReadable"
                  text="Geen machine-leesbare gegevens voor dit artikel"
                ></nldd-inline-dialog>
                <template v-else>
                  <nldd-code-editor
                    resize="auto"
                    language="yaml"
                    accessible-label="YAML"
                    :value="yamlSource"
                    @input="onYamlInput"
                  ></nldd-code-editor>
                  <nldd-banner v-if="parseError" variant="critical" :text="parseError"></nldd-banner>
                </template>
              </nldd-simple-section>

              <!-- Notities - every note for this article, one under the other,
                   plus the draft-management actions moved from the Tekst pane.
                   Deliberately minimal for now; to be fine-tuned later. -->
              <nldd-simple-section v-else-if="view === 'notes'" width="full">
                <template v-if="canCreateNotes">
                  <nldd-inline-dialog
                    v-if="noteIssues.length"
                    variant="warning"
                    :text="`${noteIssues.length} notitie(s) niet verankerd`"
                    :supporting-text="noteIssues.map(i => i.reason).join('; ')"
                  ></nldd-inline-dialog>
                  <nldd-inline-dialog
                    v-if="draftCount > 0 && !canEdit"
                    variant="warning"
                    data-testid="notes-no-traject"
                    text="Selecteer eerst een traject"
                    supporting-text="Delen werkt pas als er een actief traject is. Notities downloaden als YAML werkt wel."
                  ></nldd-inline-dialog>
                  <nldd-inline-dialog
                    v-if="notesSaveError"
                    variant="alert"
                    data-testid="notes-save-error"
                    text="Notities opslaan mislukt"
                    :supporting-text="notesSaveError"
                  ></nldd-inline-dialog>
                  <nldd-inline-dialog
                    v-if="notesSaveStatus && !notesSaveError"
                    variant="success"
                    data-testid="notes-save-status"
                    text="Opslaan gelukt"
                    :supporting-text="notesSaveStatus"
                  ></nldd-inline-dialog>
                </template>

                <nldd-inline-dialog
                  v-if="notesForArticle.length === 0"
                  text="Geen notities voor dit artikel"
                ></nldd-inline-dialog>
                <template v-else>
                  <template v-for="(group, gi) in noteGroups" :key="gi">
                    <!-- Gap between groups only; the list must not open with a
                         spacer (no leading whitespace at the top of the pane). -->
                    <nldd-spacer v-if="gi > 0" size="24"></nldd-spacer>
                    <!-- Quoted fragment (group header) as rich-text, then a
                         spacer before the cards. -->
                    <QuotedFragment v-if="group.quote" :fragment="group" />
                    <nldd-rich-text v-else>
                      <p><i>Zonder verankering</i></p>
                    </nldd-rich-text>
                    <nldd-spacer size="10"></nldd-spacer>
                    <nldd-collection layout="stack" gap="12px">
                      <nldd-card v-for="(note, ni) in group.notes" :key="ni">
                        <nldd-container padding="10">
                          <NoteCard
                            :note="note"
                            :can-edit="canEdit"
                            :saving="savingNotes"
                            @edit="startEditNote(group, note)"
                            @share="askPublishNote(note)"
                            @delete="askDeleteNote(note)"
                          />
                        </nldd-container>
                      </nldd-card>
                    </nldd-collection>
                    <!-- Add another note anchored to this same fragment, without
                         re-selecting the text in the editor. -->
                    <template v-if="canCreateNotes && group.quote">
                      <nldd-spacer size="12"></nldd-spacer>
                      <nldd-button
                        variant="secondary"
                        size="md"
                        width="full"
                        start-icon="add"
                        text="Notitie toevoegen"
                        @click="startNoteForGroup(group)"
                      ></nldd-button>
                    </template>
                  </template>
                </template>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>
        </nldd-side-by-side-split-view>
        </template>
  <!-- Overlays teleported to body: as light-DOM siblings of the split view they
       would be slotted into the main pane and pick up its ::slotted flex-grow,
       stealing height from the pane content. -->
  <Teleport to="body">
    <ActionSheet :action="activeAction" :article="editedArticle" :editable="canEdit" :is-new="activeActionIsNew" @close="handleActionClose" @save="handleActionSave" />
    <EditSheet :item="activeEditItem" :article="editedArticle" :traject-ref="activeTrajectRef" @save="handleSave" @close="activeEditItem = null" />
    <SearchPopover
      ref="searchPopoverRef"
      @select-law="onSearchSelectLaw"
      @harvest-available="onSearchHarvestAvailable"
    />
  </Teleport>

  <!-- Publishing a note is irreversible (it lands in the repo and can't be made
       private again), so confirm before the write. -->
  <nldd-modal-dialog
    ref="publishModalEl"
    text="Notitie delen?"
    supporting-text="Deze notitie wordt vastgelegd op de traject-branch in Git en is dan zichtbaar voor de andere traject-leden. Delen kan niet ongedaan worden gemaakt: de notitie kan daarna niet meer privé worden gemaakt."
    data-testid="publish-confirm"
    @close="cancelPublish"
  >
    <nldd-button slot="actions" variant="primary" text="Houd privé" @click="cancelPublish"></nldd-button>
    <nldd-button slot="actions" variant="secondary" text="Deel binnen traject" data-testid="publish-confirm-btn" @click="confirmPublish"></nldd-button>
  </nldd-modal-dialog>

  <!-- Deleting a draft is irreversible (it is the only copy), so confirm. The
       safe option (Annuleren) is the primary action. -->
  <nldd-modal-dialog
    ref="deleteModalEl"
    variant="alert"
    text="Notitie verwijderen?"
    supporting-text="Deze notitie wordt definitief verwijderd. Dit kan niet ongedaan worden gemaakt."
    data-testid="delete-confirm"
    @close="cancelDelete"
  >
    <nldd-button slot="actions" variant="primary" text="Behoud notitie" @click="cancelDelete"></nldd-button>
    <nldd-button slot="actions" variant="destructive" text="Verwijder" data-testid="delete-confirm-btn" @click="confirmDelete"></nldd-button>
  </nldd-modal-dialog>

  <!-- Whole-law save failure, shown over the whole editor rather than inline in
       one pane. -->
  <nldd-modal-dialog
    ref="saveErrorModalEl"
    variant="alert"
    text="Opslaan mislukt"
    :supporting-text="lawSaveError ? (lawSaveError.message || String(lawSaveError)) : ''"
    data-testid="save-error-modal"
    @close="dismissSaveError"
  >
    <nldd-button slot="actions" variant="primary" text="Sluiten" @click="dismissSaveError"></nldd-button>
  </nldd-modal-dialog>

  <!-- Trace sheet - execution trace + expected outcomes for the most
       recently executed scenario. Opened from a scenario card's "Toon
       resultaat" button. -->
  <nldd-sheet
    ref="resultSheetEl"
    placement="bottom"
    @close="resultSheetOpen = false"
  >
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" :text="lastScenarioName ? `Resultaat: ${lastScenarioName}` : 'Resultaat'" collapse-anchor="result-scenario-title" dismiss-text="Sluit" @dismiss="resultSheetOpen = false"></nldd-top-title-bar>
      <nldd-simple-section width="full">
        <nldd-title id="result-scenario-title" size="4"><h3>{{ lastScenarioName ? `Resultaat: ${lastScenarioName}` : 'Resultaat' }}</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>
        <ExecutionTraceView
          :result="lastResult"
          :trace-text="lastTraceText"
          :error="lastError"
          :running="lastRunning"
          :expectations="lastExpectations"
          :can-reload="!!lastReload"
          @reload="lastReload && lastReload()"
        />
      </nldd-simple-section>
    </nldd-page>
  </nldd-sheet>

  <!-- Graph sheet - visual law graph with the scenario's trace overlay.
       Opened from a scenario card's "Graaf" button. -->
  <nldd-sheet
    ref="graphSheetEl"
    placement="bottom"
    @close="graphSheetOpen = false"
  >
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" :text="lastScenarioName ? `Graaf: ${lastScenarioName}` : 'Graaf'" dismiss-text="Sluit" @dismiss="graphSheetOpen = false"></nldd-top-title-bar>
      <!-- Lazy-mount: building the Vue Flow graph for laws with many
           cross-law references is non-trivial and the sheet is hidden
           by default. Render only while the sheet is open; the remount
           cost on each subsequent open is acceptable next to dragging
           an unused Vue Flow tree behind every editor session. -->
      <LawGraphView
        v-if="graphSheetOpen"
        :law-id="lawId"
        :result="lastResult"
        :output-name="lastOutputName"
        :expectations="lastExpectations"
        :traject-ref="activeTrajectRef"
      />
    </nldd-page>
  </nldd-sheet>
</template>

<style>
.editor-engine-error {
  padding: 12px 16px;
  background: #fee;
  color: #c00;
  font-size: 13px;
}

.editor-engine-error-hint {
  margin-top: 4px;
  font-size: 12px;
  color: #999;
}

.editor-engine-error-hint code {
  background: #eee;
  padding: 1px 4px;
  border-radius: 3px;
}

.editor-parse-error {
  font-size: 12px;
  font-weight: 600;
  color: #c00;
  background: #fee;
  padding: 2px 8px;
  border-radius: 6px;
}

.editor-parse-error-detail {
  margin-top: 8px;
  background: #fef2f2;
  color: #b91c1c;
  font-family: var(--primitives-font-family-monospace);
  font-size: 12px;
  padding: 8px 12px;
  border: 1px solid #fecaca;
  border-radius: 6px;
}

</style>
