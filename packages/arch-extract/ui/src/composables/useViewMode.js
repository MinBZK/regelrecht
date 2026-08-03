/**
 * useViewMode — which of the four renderings the explorer shows.
 *
 * This is a **temporary comparison rig**: three candidate schema techniques
 * side by side, plus the original expand/collapse view as the reference to beat.
 * Once a winner is picked the loser prototypes and the `current` entry go away
 * again, and this composable with them.
 *
 * Persistence and the module-level singleton follow `useEdgeFilters.js` /
 * `useColorScheme.js` in this same directory. Keep the three in step.
 */
import { ref } from 'vue';

export const VIEW_MODES = Object.freeze([
  {
    id: 'map',
    label: 'Map',
    hint: 'Auto-layout (dagre): blokjes en lijnen, geplaatst op de verbindingen.',
  },
  {
    id: 'radial',
    label: 'Radiaal',
    hint: 'Ring per hiërarchie met hierarchical edge bundling.',
  },
  {
    id: 'matrix',
    label: 'Matrix',
    hint: 'Adjacency-matrix (DSM), geordend zodat clusters op de diagonaal komen.',
  },
  {
    id: 'current',
    label: 'Huidig',
    hint: 'De bestaande klik-gedreven weergave, als referentie.',
  },
]);

const VALID = VIEW_MODES.map((m) => m.id);
const STORAGE_KEY = 'arch-explorer-view-mode';

function read() {
  try {
    const v = window.localStorage?.getItem(STORAGE_KEY);
    return VALID.includes(v) ? v : null;
  } catch {
    return null;
  }
}

function write(value) {
  try {
    window.localStorage?.setItem(STORAGE_KEY, value);
  } catch {
    // Ignore storage errors (private mode, quota); the mode just resets on the
    // next load.
  }
}

const mode = ref((typeof window !== 'undefined' && read()) || 'map');

export function useViewMode() {
  function setViewMode(value) {
    if (!VALID.includes(value)) return;
    mode.value = value;
    write(value);
  }
  return { viewMode: mode, setViewMode, VIEW_MODES };
}
