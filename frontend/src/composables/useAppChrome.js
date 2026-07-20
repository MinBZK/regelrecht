import { shallowRef, onBeforeUnmount } from 'vue';

// Shared chrome store for the persistent AppShell.
//
// The Bibliotheek/Editor switch keeps the shell (toolbars, tab-bar,
// settings menu) mounted and only swaps the nested <router-view>. A few
// toolbar bits are view-specific — the search trigger (both views), the
// federated "PR #N" write-back indicator and the document-tab-bar (editor
// only). Rather than teleport those into the shell's toolbars — which
// `nldd-toolbar` rejects, since slotted items must be its *direct*
// children and teleport would reorder them — the active view publishes
// them here and the shell renders everything itself, in the original
// order. Module-level singleton, matching the other composables
// (`useColorScheme`, `useFeatureFlags`).

// --- Search (both views) ---
// The toolbar search button/field lives in the shell; the SearchPopover it
// opens stays in each view (its `@select-law` handling differs). The view
// registers its popover ref; the shell calls openSearch with the click/key
// event so the popover anchors to the bar control.
let popoverRef = null;

export function registerSearchPopover(ref_) {
  popoverRef = ref_;
  // Clear on unmount, but only if this view still owns the registration. The
  // entering view registers in its setup before the leaving view unmounts, so
  // guarding on identity avoids nulling a registration that already moved on —
  // making the cleanup safe even if the route topology changes that ordering.
  onBeforeUnmount(() => {
    if (popoverRef === ref_) popoverRef = null;
  });
}

export function openSearch(e, initialSearch = '') {
  popoverRef?.value?.show(e?.currentTarget, initialSearch);
}

/**
 * Spotlight-style: any printable single-character keystroke on the bar's
 * search-field opens the popover with that character as the initial query.
 * Modifier-combos (Ctrl-A, Cmd-V, …), Tab, Enter, arrows fall through.
 */
export function onBarSearchKeydown(e) {
  if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
    e.preventDefault();
    openSearch(e, e.key);
  }
}

// --- Editor-only chrome ---
// Federated write-back indicator ({ number, url } | null).
const lastSavedPr = shallowRef(null);
// Open document tabs + the active one, plus the actions to drive them. The
// editor keeps these in sync via a watchEffect; the shell renders the
// `document-tabs` pane only while `documentTabs` is non-empty, so the
// library never shows an empty tab bar.
const documentTabs = shallowRef([]);
const activeDocumentTab = shallowRef(null);
const tabActions = shallowRef(null); // { key, displayName, select, close, reorder }
// Article-level pending-changes bar (Wijzigingenbalk). The editor publishes
// the dirty/saving/undo state reactively and registers the action callbacks;
// the shell renders the bar while there are unsaved changes.
const editorChanges = shallowRef(null); // { dirty, saving, canUndo, canRedo } | null
const editorActions = shallowRef(null); // { save, discard, undo, redo } | null

export function useAppChrome() {
  return {
    lastSavedPr,
    documentTabs,
    activeDocumentTab,
    tabActions,
    editorChanges,
    editorActions,
  };
}

export function setEditorChrome({ pr, tabs, activeTab }) {
  lastSavedPr.value = pr ?? null;
  documentTabs.value = tabs ?? [];
  activeDocumentTab.value = activeTab ?? null;
}

export function registerTabActions(actions) {
  tabActions.value = actions;
}

// Reactive snapshot of the article's pending-changes state. Called from a
// watchEffect in the editor so a new object is published whenever dirty,
// saving or undo-availability changes, re-rendering the bar.
export function setEditorChanges(state) {
  editorChanges.value = state ?? null;
}

// Stable action callbacks for the changes bar; registered once.
export function registerEditorActions(actions) {
  editorActions.value = actions ?? null;
}

// Called when the editor view unmounts so the shell drops the editor-only
// chrome (PR badge, document tabs, changes bar) while the library is mounted.
export function clearEditorChrome() {
  lastSavedPr.value = null;
  documentTabs.value = [];
  activeDocumentTab.value = null;
  tabActions.value = null;
  editorChanges.value = null;
  editorActions.value = null;
}
