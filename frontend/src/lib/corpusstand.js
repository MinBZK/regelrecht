/**
 * corpusstand.js — aggregatie over de notitie-sidecars van een traject
 * (bouwplan §3.2, "verklaarde bevindingen").
 *
 * Pure functies zonder Vue/DOM/fetch, zodat het rekenwerk zonder draaiende
 * stack te testen is. De composable eromheen (`useCorpusstand.js`) doet het
 * netwerkwerk en voedt deze functies.
 *
 * ## Waarom hier wél uit de sidecar gelezen wordt
 *
 * Het bouwplan (§2) verbiedt een tweede parser naast de engine, omdat een
 * afwijkend cijfer erger is dan geen cijfer. Dat verbod geldt voor velden die
 * de engine *interpreteert*. Voor noten valt de scheidslijn zo:
 *
 * - `motivation`, `workflow`, `creator` en tagging-bodies worden door de
 *   engine niet gelezen. Er is geen engine-model om van af te wijken, dus
 *   tellen uit de sidecar kan geen divergentie opleveren.
 * - `resolution` (found/orphaned/ambiguous) *is* een engine-uitspraak: hij
 *   komt uit `resolveNotes()`, dat de TextQuoteSelector tegen de wettekst
 *   houdt. Die waarde nemen we daarom nooit uit de sidecar over — het veld
 *   staat er wel in, maar het is een gecachete mening die verouderd kan zijn
 *   zodra de wettekst wijzigt. Precies dat verschil is de bevinding.
 *
 * ## Determinisme
 *
 * Bouwplan §5: gelijke invoer levert gelijke uitvoer. Elke sortering hier is
 * totaal (aantal aflopend, dan sleutel oplopend) zodat er geen twee rijen zijn
 * waarvan de volgorde van de iteratievolgorde van een Map afhangt.
 */

/**
 * @typedef {Object} WetInvoer
 * @property {string} lawId
 * @property {object[]} notes  ruwe annotatie-objecten uit de sidecar
 * @property {Array<{status: string}>|null} ankers  per noot (zelfde index) de
 *   uitspraak van de engine-resolver, of `null` als er niet geresolved is
 *
 * **Uitlijning is een contract van de aanroeper.** `resolveNotes()` slaat noten
 * over die een andere wet targeten (een sidecar mag noten voor meerdere wetten
 * dragen), dus zijn uitvoer is korter dan de `annotations`-lijst in het bestand.
 * `useCorpusstand.js` lost dat op door bij een geslaagde resolve *beide* velden
 * uit de resolver-uitvoer op te bouwen — dan zijn ze per constructie uitgelijnd
 * én correct afgebakend tot de noten die bij deze wet horen. Alleen als de
 * resolver niet kon draaien komt `notes` uit het bestand, met `ankers: null`.
 */

/**
 * Alle `TextualBody`-waarden met `purpose: tagging`.
 *
 * Spiegelt `tagging_values()` in `packages/engine/src/bin/validate_annotations.rs`
 * — inclusief het gegeven dat `body` zowel een enkel object als een array mag
 * zijn. Wijkt dit af, dan telt het dashboard andere tags dan CI valideert.
 *
 * @param {object} note
 * @returns {string[]}
 */
export function tagWaarden(note) {
  const body = note?.body;
  const bodies = Array.isArray(body) ? body : body ? [body] : [];
  const out = [];
  for (const b of bodies) {
    if (b?.purpose === 'tagging' && typeof b.value === 'string') out.push(b.value);
  }
  return out;
}

/**
 * Staat deze noot nog open? Het schema geeft `workflow` de default `open`, dus
 * een ontbrekend veld telt als open — niet als onbekend.
 */
export function isOpen(note) {
  return (note?.workflow ?? 'open') === 'open';
}

/** Het artikelnummer waar een noot aan hangt, als de selector-hint het draagt. */
export function artikelVanNoot(note) {
  return note?.target?.selector?.hint?.article_number ?? null;
}

/** De aangehaalde tekst, voor het herkennen van een losgeraakte noot. */
export function exactVanNoot(note) {
  return note?.target?.selector?.exact ?? null;
}

/**
 * Tel voorkomens en geef ze terug als stabiel gesorteerde rijen.
 * Aantal aflopend, bij gelijk aantal de sleutel oplopend.
 */
function telEnSorteer(waarden) {
  const tellingen = new Map();
  for (const w of waarden) tellingen.set(w, (tellingen.get(w) ?? 0) + 1);
  return [...tellingen.entries()]
    .map(([key, n]) => ({ key, n }))
    .sort((a, b) => b.n - a.n || a.key.localeCompare(b.key, 'nl'));
}

/**
 * Bouw het bevindingen-rapport over alle wetten van een traject.
 *
 * @param {WetInvoer[]} perWet  alleen wetten mét een sidecar; een wet zonder
 *   noten levert niets op om te tellen en hoort er niet in
 * @param {Array<{id: string, label: string}>} vocabulaire  `ambiguity.yaml`
 * @param {{wettenInCorpus?: number|null}} opts  het totaal aantal wetten in
 *   het corpus. Staat los van `perWet` omdat alleen de aanroeper dat weet:
 *   zonder dit getal leest "noten over 3 wetten" als volledige dekking,
 *   terwijl het over 3 van de 24 kan gaan. Dat verschil is de metriek.
 * @returns {object} rapport
 */
export function aggregeer(perWet, vocabulaire = [], { wettenInCorpus = null } = {}) {
  const bekend = new Map(vocabulaire.map((v) => [v.id, v.label]));

  let totaal = 0;
  let open = 0;
  const soorten = [];
  const tags = [];
  const ankerfouten = [];
  const wetRijen = [];

  // `perWet` op lawId sorteren maakt de uitvoer onafhankelijk van de volgorde
  // waarin de fetches terugkwamen — zonder dit verschilt het rapport per run.
  const gesorteerd = [...perWet].sort((a, b) => a.lawId.localeCompare(b.lawId, 'nl'));

  for (const wet of gesorteerd) {
    const notes = wet.notes ?? [];
    let wetOpen = 0;

    notes.forEach((note, i) => {
      totaal += 1;
      if (isOpen(note)) {
        open += 1;
        wetOpen += 1;
      }
      if (typeof note?.motivation === 'string') soorten.push(note.motivation);
      tags.push(...tagWaarden(note));

      // Ankerstatus komt uitsluitend van de engine. Geen resolver-uitslag
      // betekent "niet gemeten", niet "in orde" — anders leest een traject
      // waar de engine niet kon laden als een gezond traject.
      const status = wet.ankers?.[i]?.status;
      if (status === 'orphaned' || status === 'ambiguous') {
        ankerfouten.push({
          lawId: wet.lawId,
          status,
          artikel: artikelVanNoot(note),
          exact: exactVanNoot(note),
        });
      }
    });

    if (notes.length > 0) {
      wetRijen.push({ lawId: wet.lawId, n: notes.length, open: wetOpen, gemeten: wet.ankers != null });
    }
  }

  const naarTag = telEnSorteer(tags).map(({ key, n }) => ({
    id: key,
    label: bekend.get(key) ?? key,
    n,
    inVocabulaire: bekend.has(key),
  }));

  return {
    totaal,
    open,
    wettenMetSidecar: gesorteerd.length,
    wettenMetNoten: wetRijen.length,
    wettenInCorpus,
    naarSoort: telEnSorteer(soorten),
    naarTag,
    buitenVocabulaire: naarTag.filter((t) => !t.inVocabulaire).reduce((s, t) => s + t.n, 0),
    ankerfouten: {
      orphaned: ankerfouten.filter((a) => a.status === 'orphaned').length,
      ambiguous: ankerfouten.filter((a) => a.status === 'ambiguous').length,
      // Sortering: wet, dan artikel, dan de aangehaalde tekst. Artikelnummers
      // zijn strings ('2', '10', '2a') en worden numeriek-bewust vergeleken,
      // zodat artikel 10 na artikel 2 komt in plaats van ervoor.
      items: ankerfouten.sort(
        (a, b) =>
          a.lawId.localeCompare(b.lawId, 'nl') ||
          (a.artikel ?? '').localeCompare(b.artikel ?? '', 'nl', { numeric: true }) ||
          (a.exact ?? '').localeCompare(b.exact ?? '', 'nl'),
      ),
    },
    // Niet gemeten wetten apart houden: het verschil tussen "geen ankerfouten"
    // en "niet gekeken" is het hele punt van deze metriek.
    ongemeten: wetRijen.filter((w) => !w.gemeten).map((w) => w.lawId),
    perWet: wetRijen.sort((a, b) => b.n - a.n || a.lawId.localeCompare(b.lawId, 'nl')),
  };
}
