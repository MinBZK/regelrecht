/**
 * De kroniek van een cel vanaf de achterkant: elke gram zoals de cel hem
 * opslaat, in de volgorde waarin hij is vastgelegd, met de registratie in de
 * stroom die hem liet ontstaan. Generiek: welke gebeurtenis, welk veld of
 * welke wet het is, weet dit bestand niet; dat staat in de grammen, de
 * stromen van de cel en de vorm die de cel uit de wet afleidt.
 */

/** De gebeurtenis in de stromen van `cell` die `gram` liet ontstaan, of null. */
export function streamEventOf(cell, gram) {
  return (cell?.events ?? []).find((e) => e.name === gram?.name && e.chronicle === gram?.chronicle) ?? null;
}

/**
 * De gram waar de zaak van `gram` mee begon: volg steeds de eerste
 * verwijzing (op naam, alfabetisch) naar de gram zonder verwijzing. Wijst die
 * eerste verwijzing naar een gram die er niet is, dan houdt de weg daar op.
 * `byId` bevat de grammen van één kroniek, die van `gram`.
 * Precies zoals de cel het doet (`Chronicle::root_of`), zodat de demo een
 * gram bij dezelfde zaak zet als de cel; ook de grens op het aantal stappen
 * is die van de cel.
 */
export function rootOf(gram, byId) {
  let current = gram;
  if (!current) return null;
  for (let i = 0; i <= byId.size; i += 1) {
    const refs = current.refers_to ?? {};
    const first = Object.keys(refs).sort()[0];
    const next = first === undefined ? undefined : byId.get(refs[first]);
    if (!next) break;
    current = next;
  }
  return current.id;
}

/** Een moment (RFC 3339) als getal om op te sorteren; ongeldig achteraan. */
function instant(moment) {
  const ms = Date.parse(moment ?? '');
  return Number.isNaN(ms) ? Number.POSITIVE_INFINITY : ms;
}

/**
 * De grammen van `cell` (die in een kroniek van haar stromen) in de volgorde
 * van vastleggen (`recorded_at`); bij gelijke tijd in de volgorde waarin de
 * cel ze geeft. Per gram zijn plaats (`position`, vanaf 1), de gebeurtenis in
 * de stroom, de gram waar zijn zaak mee begon, en elke verwijzing met de
 * plaats van de gram waarnaar zij wijst (`null` als die er niet is).
 */
export function storedChronicle(cell, grams) {
  const chronicles = new Set((cell?.events ?? []).map((e) => e.chronicle));
  const own = (grams ?? [])
    .map((gram, i) => ({ gram, i }))
    .filter(({ gram }) => chronicles.has(gram.chronicle))
    .sort((a, b) => instant(a.gram.recorded_at) - instant(b.gram.recorded_at) || a.i - b.i)
    .map(({ gram }) => gram);
  // De weg naar de eerste gram van de zaak blijft binnen één kroniek, zoals
  // bij de cel (`Chronicle::root_of`): een verwijzing naar een gram in een
  // andere kroniek van dezelfde cel houdt daar op.
  const byChronicle = new Map();
  for (const g of own) {
    if (!byChronicle.has(g.chronicle)) byChronicle.set(g.chronicle, new Map());
    byChronicle.get(g.chronicle).set(g.id, g);
  }
  const position = new Map(own.map((g, i) => [g.id, i + 1]));
  return own.map((gram, i) => ({
    gram,
    position: i + 1,
    event: streamEventOf(cell, gram),
    root: rootOf(gram, byChronicle.get(gram.chronicle)),
    references: Object.entries(gram.refers_to ?? {}).map(([role, id]) => ({ role, id, position: position.get(id) ?? null })),
  }));
}

/** Alleen de grammen van de zaak die met gram `root` begon; zonder `root` alles. */
export function onlyCase(entries, root) {
  return root ? entries.filter((e) => e.root === root) : entries;
}

/**
 * De velden van een gebeurtenis per artikel dat erom vraagt (`declared_by`
 * uit de vorm die de cel afleidt): het artikel dat de gram vestigt en elke
 * haak die er velden aan toevoegt, in de volgorde waarin ze voor het eerst
 * voorkomen.
 */
export function fieldsByArticle(fields) {
  const groups = new Map();
  for (const f of fields ?? []) {
    const article = f.declared_by ?? '';
    if (!groups.has(article)) groups.set(article, []);
    groups.get(article).push(f.name);
  }
  return [...groups].map(([article, names]) => ({ article, names }));
}

/**
 * De herkomst van een invoer van een besluit zoals de cel haar vastlegt
 * (`{source, ...}`), als één regel: elk onderdeel als `sleutel: waarde`.
 */
export function provenanceText(provenance) {
  if (provenance == null) return '';
  if (typeof provenance !== 'object') return String(provenance);
  return Object.entries(provenance)
    .map(([k, v]) => `${k}: ${typeof v === 'object' && v !== null ? JSON.stringify(v) : v}`)
    .join(', ');
}
