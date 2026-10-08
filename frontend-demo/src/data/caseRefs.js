/**
 * Wat een wet uit de besloten zaken van een andere wet leest.
 *
 * Los van graph/lawGraph.js en zonder afhankelijkheden: de corpuslader zet dit
 * bij elke wet, en zou anders de graafbibliotheek in de hoofdbundel trekken.
 */

function executions(law) {
  return (law.doc?.articles ?? []).map((a) => a.machine_readable?.execution).filter(Boolean);
}

/**
 * The inputs a law reads from another law's decided cases (`kind: cases` with
 * `field` in bindings.yaml), as references to that law's output of the same
 * name. Precario reads the granted area from the terrace permit that way; in
 * the law itself that input has `source: {}`, so without this the graph drew
 * no line between the two.
 *
 * @param {{id: string}} law
 * @param {Record<string, Record<string, object>>} bindings
 * @param {(lawPath: string, service?: string) => {id: string}|null} lawByPath
 * @returns {{name: string, regulation: string, output: string}[]}
 */
export function caseRefsFor(law, bindings, lawByPath) {
  const out = [];
  for (const [name, b] of Object.entries(bindings?.[law.id] ?? {})) {
    if (b.kind !== 'cases' || !b.field) continue;
    const path = (b.select_on ?? []).find((s) => s.name === 'law')?.value;
    const supplier = path ? lawByPath(path, b.service) : null;
    // Alleen een veld dat de andere wet ook uitrekent; `approved` is een
    // feit van de zaak, geen uitkomst van de wet.
    const produces = supplier && executions(supplier).some((ex) => (ex.output ?? []).some((o) => o.name === b.field));
    if (produces) out.push({ name, regulation: supplier.id, output: b.field });
  }
  return out;
}
