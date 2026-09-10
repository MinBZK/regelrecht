// Fails the build when a template assigns a `slot="..."` that the enclosing
// design-system component does not define.
//
// Such an assignment is invisible in every other way: no console error, no
// build warning. The element stays in the light DOM, is never assigned to a
// shadow-DOM slot, and lays out as 0x0. The persona tags on the demo portal
// and the "Start de presentatie" button both sat on `slot="actions"` of
// `nldd-title` — which has `overline`, `subtitle` and `end` — and neither was
// on screen for as long as they had been there.
//
//   node script/check-nldd-slots.mjs <source-dir> [<source-dir>...]

import { componentSlots, slotAssignments, unknownSlots } from './nldd-slots.mjs';

const dirs = process.argv.slice(2);
if (dirs.length === 0) {
  console.error('usage: check-nldd-slots.mjs <source-dir> [<source-dir>...]');
  process.exit(2);
}

const PACKAGE_DIR = 'node_modules/@nldd/design-system/dist/components';
const slots = componentSlots(PACKAGE_DIR);
if (slots.size === 0) {
  console.error(`✗ geen componenten gevonden in ${PACKAGE_DIR} — is het ontwerpsysteem geïnstalleerd?`);
  process.exit(2);
}

let failures = 0;
for (const dir of dirs) {
  const bad = unknownSlots(slotAssignments(dir), slots);
  for (const { file, line, parent, child, slot } of bad) {
    const known = [...slots.get(parent)].sort();
    console.error(
      `✗ ${file}:${line}: <${child} slot="${slot}"> in <${parent}>, dat die slot niet heeft.\n` +
        `    ${parent} kent: ${known.length ? known.join(', ') : '(alleen de standaardslot)'}`,
    );
    failures += 1;
  }
}

if (failures > 0) {
  console.error(`\n${failures} slot${failures === 1 ? '' : 's'} die het ontwerpsysteem niet kent. Zo'n element is 0x0 en dus onzichtbaar.`);
  process.exit(1);
}

console.log(`✓ alle slot-toewijzingen bestaan (${dirs.join(', ')})`);
