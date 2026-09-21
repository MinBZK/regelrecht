// Which slots each nldd-* component actually defines, and which ones a source
// tree assigns to. Shared by check-nldd-slots.mjs and its test.
//
// The reason this needs checking at all: a `slot="..."` that a component does
// not define is not an error anywhere. The element stays in the light DOM,
// never gets assigned to a shadow-DOM slot, and lays out as 0x0 — present in
// the DOM, invisible on screen, silent in the console. Two of those sat in the
// demo unnoticed: the persona tags on the portal and the "Start de
// presentatie" button, both on `slot="actions"` of `nldd-title`, which only
// has `overline`, `subtitle` and `end`.

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

// .rs for the poc portal, whose pages are markup in Rust strings.
export const EXTENSIONS = ['.vue', '.html', '.js', '.ts', '.rs'];

// The attributes of a tag, up to its closing `>`. A quoted value is taken
// whole, because Vue expressions carry comparisons: `[^>]*` stopped at the `>`
// in `v-if="n > 1"`, so a slot after it was never seen.
const ATTRIBUTES = String.raw`(?:[^>"']|"[^"]*"|'[^']*')*`;
const SLOT_ASSIGNMENT = new RegExp(
  String.raw`<([a-z][a-z0-9-]*)\b(${ATTRIBUTES}?)\bslot=["']([a-z0-9-]+)["']`,
  'gi',
);
const TAG = new RegExp(String.raw`<(\/?)([a-z][a-z0-9-]*)\b(${ATTRIBUTES})>`, 'gi');

/**
 * Every file under `dir` with one of these extensions. A directory that is not
 * there yields nothing: the design-system package is only present after
 * `npm ci`, and this guard has to stay silent where it is not, not crash.
 */
export function sourceFiles(dir, extensions = EXTENSIONS, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === 'dist' || entry.startsWith('.')) continue;
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) sourceFiles(full, extensions, out);
    else if (extensions.some((e) => entry.endsWith(e))) out.push(full);
  }
  return out;
}

/**
 * The slot names a component's template declares, per tag.
 *
 * Templates are `*.template.js` files in the design-system package that render
 * `<slot name="x">`; a component with only a default slot has an entry with an
 * empty set, which is what makes "this tag takes no named slots" checkable.
 *
 * A component whose template builds slot names at runtime (`<slot name=${...}>`)
 * accepts any name by design — `nldd-bar-split-view` names a slot after each
 * child it is given, which is how a page adds its own toolbars. Those map to
 * `null`, meaning "do not check this tag", rather than to a set that would
 * reject every real usage.
 *
 * A name with a fixed part around the expression (`<slot name="pane-${n}">`,
 * how `nldd-side-by-side-split-view` numbers its panes) is kept as a pattern:
 * `pane-*`. That accepts `pane-2` and still rejects a typo like `paneel-2`.
 */
export function componentSlots(packageDir) {
  const slots = new Map();
  for (const file of sourceFiles(packageDir, ['.template.js'])) {
    // .../components/<group>/<tag>/<tag>.template.js
    const match = /([^/]+)\.template\.js$/.exec(file);
    if (!match) continue;
    const tag = `nldd-${match[1]}`;
    const text = readFileSync(file, 'utf8');
    if (/<slot\s+name=\$\{/.test(text)) {
      slots.set(tag, null);
      continue;
    }
    const names = new Set();
    for (const m of text.matchAll(/<slot\s+name=["']((?:[a-z0-9-]|\$\{[^}]*\})+)["']/gi)) {
      names.add(m[1].replace(/\$\{[^}]*\}/g, '*'));
    }
    // A component can also forward a slot it received; those show up as
    // `slot="x"` on an element inside its own template.
    for (const m of text.matchAll(/\bslot=["']([a-z0-9-]+)["']/gi)) names.add(m[1]);
    slots.set(tag, names.has('*') ? null : names);
  }
  return slots;
}

/** Whether `slot` is one of `known`, literally or through a `pane-*` pattern. */
function definesSlot(known, slot) {
  if (known.has(slot)) return true;
  return [...known].some(
    (name) => name.includes('*') && new RegExp(`^${name.replaceAll('*', '.+')}$`).test(slot),
  );
}

/**
 * Every `slot="name"` assignment in a source tree, with the nldd-* parent it
 * lands in.
 *
 * The parent is the nearest enclosing element, found by scanning backwards for
 * the last unclosed opening tag. That is a heuristic, not a parser: it is
 * accurate for the shape these templates actually have (an nldd-* element with
 * slotted children on their own lines) and is deliberately conservative —
 * when the parent cannot be determined the assignment is skipped rather than
 * reported, so this never invents a failure.
 */
export function slotAssignments(dir, extensions = EXTENSIONS) {
  const found = [];
  for (const file of sourceFiles(dir, extensions)) {
    const text = readFileSync(file, 'utf8');
    for (const m of text.matchAll(SLOT_ASSIGNMENT)) {
      const parent = enclosingTag(text, m.index);
      if (!parent?.startsWith('nldd-')) continue;
      found.push({
        file,
        line: text.slice(0, m.index).split('\n').length,
        parent,
        child: m[1].toLowerCase(),
        slot: m[3],
      });
    }
  }
  return found;
}

/** The tag of the nearest element still open at `index`. */
function enclosingTag(text, index) {
  const before = text.slice(0, index);
  const stack = [];
  for (const m of before.matchAll(TAG)) {
    const [, closing, tag, attrs] = m;
    if (closing) {
      const at = stack.lastIndexOf(tag.toLowerCase());
      if (at !== -1) stack.length = at;
    } else if (!attrs.trimEnd().endsWith('/')) {
      stack.push(tag.toLowerCase());
    }
  }
  return stack[stack.length - 1] ?? null;
}

/**
 * Slot assignments whose parent component does not define that slot.
 *
 * A tag the package does not describe is skipped: it is either not a
 * design-system component or one whose template this scan cannot see, and
 * guessing there would produce noise instead of findings. So is a component
 * that names its slots at runtime (`null`), which accepts anything.
 */
export function unknownSlots(assignments, slots) {
  return assignments.filter(({ parent, slot }) => {
    const known = slots.get(parent);
    return known != null && !definesSlot(known, slot);
  });
}
