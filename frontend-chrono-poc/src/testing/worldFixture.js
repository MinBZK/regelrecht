/**
 * Het beeld waarop de tests draaien.
 *
 * Dit is de fixture van de simulator zelf (`packages/simulator/tests/fixtures/`),
 * niet een kopie ernaast: het beeld is het contract tussen de wereld en deze app,
 * en twee exemplaren zouden stil uit elkaar lopen. Verandert de vorm, dan valt
 * deze app om in de test en niet in de browser.
 *
 * De wereld erin is de publieke testwereld van het platform. Geen casus.
 */
import snapshot from '../../../packages/simulator/tests/fixtures/snapshot.json';

/** Het beeld zoals de fixture het geeft. Niet wijzigen; gebruik `cloneWorld`. */
export const worldFixture = snapshot;

/** Een eigen exemplaar, voor een test die het beeld aanpast. */
export function cloneWorld() {
  return structuredClone(snapshot);
}

/** Eén cel uit de fixture, op id. */
export function fixtureCell(id) {
  return worldFixture.cells.find((cell) => cell.id === id);
}

/** Het eerste gram van een soort in een cel, met zijn kroniek. */
export function fixtureGram(cellId, kind) {
  for (const chronicle of fixtureCell(cellId).chronicles) {
    const gram = chronicle.grams.find((candidate) => candidate.kind === kind);
    if (gram) return { chronicle, gram };
  }
  return null;
}
