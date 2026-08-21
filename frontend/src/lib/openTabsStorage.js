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
 *
 * Reads are sanitised (see `sanitizeTabs`): earlier builds of the per-traject
 * feature briefly wrote one traject's law under another traject's key, and
 * nothing validated the stored content, so that pollution outlived the write
 * fix. Sanitising on read (drop malformed entries, normalise `articleNumber`
 * to a string, de-duplicate, cap at `MAX_TABS`) heals such storage the moment
 * it is loaded.
 */

const TABS_STORAGE_PREFIX = 'regelrecht-open-tabs:';
const ACTIVE_TAB_STORAGE_PREFIX = 'regelrecht-active-tab:';

// Cap on how many law tabs a traject keeps open; the oldest are dropped past
// this. Lives here (rather than in EditorView) so both the sanitising read and
// the in-memory store (`useOpenTabs`) enforce the same bound.
export const MAX_TABS = 20;

function tabsKey(trajectRef) {
  return `${TABS_STORAGE_PREFIX}${trajectRef ?? ''}`;
}

function activeTabKey(trajectRef) {
  return `${ACTIVE_TAB_STORAGE_PREFIX}${trajectRef ?? ''}`;
}

/**
 * Coerce one stored value into a valid tab `{ lawId, articleNumber }` or
 * `null`. A tab needs a non-empty string `lawId` and an `articleNumber`; the
 * latter is normalised to a string so a legacy numeric value and a string
 * value never read as two different tabs.
 */
export function sanitizeTab(raw) {
  if (!raw || typeof raw !== 'object') return null;
  const { lawId, articleNumber } = raw;
  if (typeof lawId !== 'string' || lawId === '') return null;
  if (articleNumber == null || articleNumber === '') return null;
  return { lawId, articleNumber: String(articleNumber) };
}

/**
 * Sanitise a parsed tab array: drop malformed entries, normalise
 * `articleNumber` to a string, de-duplicate by `lawId:articleNumber`
 * (first occurrence wins) and cap at `MAX_TABS` (keeping the newest).
 */
export function sanitizeTabs(parsed) {
  if (!Array.isArray(parsed)) return [];
  const seen = new Set();
  const clean = [];
  for (const raw of parsed) {
    const tab = sanitizeTab(raw);
    if (!tab) continue;
    const key = `${tab.lawId}:${tab.articleNumber}`;
    if (seen.has(key)) continue;
    seen.add(key);
    clean.push(tab);
  }
  return clean.length > MAX_TABS ? clean.slice(-MAX_TABS) : clean;
}

/** The saved open tabs for a traject, or `[]` when there are none / on error. */
export function loadSavedTabs(trajectRef) {
  try {
    const saved = localStorage.getItem(tabsKey(trajectRef));
    return sanitizeTabs(saved ? JSON.parse(saved) : []);
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
    return sanitizeTab(saved ? JSON.parse(saved) : null);
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
