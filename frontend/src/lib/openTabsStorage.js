/**
 * Per-traject persistence for the editor's open law tabs.
 *
 * The editor's tab bar is scoped to the active traject: each traject remembers
 * its own set of open tabs and which one was active, so switching trajects
 * swaps the bar to that traject's set instead of carrying over tabs that point
 * at laws the new traject doesn't have.
 *
 * Keys follow the per-entity `regelrecht-<feature>:<id>` convention used
 * elsewhere (cf. `useDraftNotes.js`), here `<id>` is the traject ref. All
 * access is wrapped in try/catch with a safe default so full/disabled storage
 * never breaks the editor - tab persistence is best-effort.
 *
 * A tab has the shape `{ lawId, articleNumber }`.
 */

const TABS_STORAGE_PREFIX = 'regelrecht-open-tabs:';
const ACTIVE_TAB_STORAGE_PREFIX = 'regelrecht-active-tab:';

function tabsKey(trajectRef) {
  return `${TABS_STORAGE_PREFIX}${trajectRef ?? ''}`;
}

function activeTabKey(trajectRef) {
  return `${ACTIVE_TAB_STORAGE_PREFIX}${trajectRef ?? ''}`;
}

/** The saved open tabs for a traject, or `[]` when there are none / on error. */
export function loadSavedTabs(trajectRef) {
  try {
    const saved = localStorage.getItem(tabsKey(trajectRef));
    const parsed = saved ? JSON.parse(saved) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

/** Persist a traject's open tabs (best-effort). */
export function saveTabs(trajectRef, tabs) {
  try {
    localStorage.setItem(tabsKey(trajectRef), JSON.stringify(tabs));
  } catch {
    /* quota/full or disabled - tabs are best-effort */
  }
}

/** The saved active tab for a traject, or `null` when none / on error. */
export function loadSavedActiveTab(trajectRef) {
  try {
    const saved = localStorage.getItem(activeTabKey(trajectRef));
    return saved ? JSON.parse(saved) : null;
  } catch {
    return null;
  }
}

/** Persist (or clear, when `tab` is falsy) a traject's active tab. */
export function saveActiveTab(trajectRef, tab) {
  try {
    if (!tab) localStorage.removeItem(activeTabKey(trajectRef));
    else localStorage.setItem(activeTabKey(trajectRef), JSON.stringify(tab));
  } catch {
    /* quota/full or disabled - tabs are best-effort */
  }
}
