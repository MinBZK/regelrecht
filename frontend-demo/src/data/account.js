/**
 * De rekening van een persona bij de fictieve bank, zoals het portaal haar
 * toont: het saldo en de overboekingen. Wat er op de rekening is gebeurd,
 * staat in de kroniek van de bankcel; het beginsaldo en het rekeningnummer
 * in de gegevens van de persona (profiles.yaml).
 *
 * Een afschrift is een weergave, geen wet: de bank telt bij het beginsaldo
 * op wat zij bijschreef en trekt af wat zij afschreef. Welke cel, welke tabel
 * en welke velden dat zijn, staat onder `account` in demo-config.yaml; dit
 * bestand kent ze niet bij naam. Een veld mag een lijst namen zijn: grammen
 * van verschillende soort (een overboeking, een incasso) dragen het kenmerk
 * onder een andere naam, en de eerste die de gram heeft telt.
 */

/** Het veld `field` (een naam of een lijst namen) van gram `g`: de eerste die er is. */
function fieldOf(g, field) {
  for (const name of Array.isArray(field) ? field : [field]) {
    const value = name == null ? undefined : g.fields?.[name];
    if (value !== undefined && value !== null && value !== '') return value;
  }
  return undefined;
}

/**
 * De rekening van de persona met `sources` (zijn gegevens per organisatie en
 * tabel) volgens `config` (`account` in demo-config.yaml), met de grammen
 * `grams` van alle cellen. `null` als de persona daar geen rekening heeft.
 *
 * Geeft `{ number, blocked, opening, balance, transactions }`; een transactie
 * is `{ id, name, date, amount, credited, debited, reason, reference, payer }`,
 * de nieuwste eerst. `payer` is de cel die de gram vastlegde waar het kenmerk
 * naar wijst (de opdrachtgever of de schuldeiser), als die er is.
 */
export function accountOf(config, sources, grams, cells = []) {
  if (!config) return null;
  const data = config.data ?? {};
  const row = (sources?.[data.service]?.[data.table] ?? [])[0];
  if (!row) return null;
  const number = row[data.number];
  const fields = config.gram ?? {};
  const chronicles = new Set((cells.find((c) => c.id === config.cell)?.events ?? []).map((e) => e.chronicle));
  const byId = new Map((grams ?? []).map((g) => [g.id, g]));
  const payerOf = (id) => {
    const g = byId.get(id);
    if (!g) return null;
    return cells.find((c) => c.events.some((e) => e.chronicle === g.chronicle))?.recordingActor ?? null;
  };
  const transactions = (grams ?? [])
    .filter((g) => chronicles.has(g.chronicle) && fieldOf(g, fields.account) === number)
    .map((g) => ({
      id: g.id,
      name: g.name,
      date: fieldOf(g, fields.date) ?? g.effective_at?.slice(0, 10) ?? null,
      amount: fieldOf(g, fields.amount) ?? null,
      credited: Number(fieldOf(g, fields.credited) ?? 0),
      debited: Number(fieldOf(g, fields.debited) ?? 0),
      reason: fieldOf(g, fields.reason) || null,
      reference: fieldOf(g, fields.reference) ?? null,
      payer: payerOf(fieldOf(g, fields.reference)),
      recorded: g.recorded_at ?? '',
    }))
    .sort((a, b) => (b.date ?? '').localeCompare(a.date ?? '') || b.recorded.localeCompare(a.recorded));
  const opening = Number(row[data.opening_balance] ?? 0);
  const balance = transactions.reduce((sum, t) => sum + t.credited - t.debited, opening);
  return { number, blocked: row[data.blocked] === true, opening, balance, transactions };
}
