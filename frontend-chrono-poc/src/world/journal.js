/**
 * Het journaal lezen: het verhaal van de wereld, in tijdsvolgorde.
 *
 * Het contract is `Snapshot::journal` uit `packages/simulator/src/journal.rs`.
 * Deze module zet die regels om in wat de weergave nodig heeft — wie deed het,
 * welke grammen kwamen eruit, wat veranderde er — en kent geen casus: elke naam
 * die een bezoeker ziet, staat in het beeld.
 *
 * Wat hier níet gebeurt is rekenen. Het verschil tussen "was" en "is" is door de
 * wereld gemeten, op de kronieken zelf; deze app zou dat niet kunnen en hoort het
 * ook niet te proberen.
 */
import { formatValue } from './format.js';
import { cells, describeZaak, gramByRef } from './snapshot.js';

/**
 * De soorten gebeurtenis, met hoe ze eruitzien.
 *
 * Eén vocabulaire voor de hele pagina: dezelfde kleuren als de grammen waar dat
 * over hetzelfde gaat (een besluit is donkerblauw, zoals het decretogram dat
 * eruit komt), en een eigen kleur waar het iets anders is.
 */
export const JOURNAL_KINDS = {
  vastlegging: { label: 'Vastlegging', color: 'groen', icon: 'file-text' },
  besluit: { label: 'Besluit', color: 'donkerblauw', icon: 'certificate' },
  betaling: { label: 'Betaling', color: 'oranje', icon: 'euro-sign' },
  termijn: { label: 'Termijn', color: 'warning', icon: 'warning' },
  vraag: { label: 'Vraag over de celgrens', color: 'hemelblauw', icon: 'question' },
};

const UNKNOWN_KIND = { label: 'Gebeurtenis', color: 'neutral', icon: 'circle' };

/** Hoe deze soort gebeurtenis eruitziet; een onbekende soort blijft leesbaar. */
export function journalKind(kind) {
  return JOURNAL_KINDS[kind] ?? UNKNOWN_KIND;
}

/** De klok als actor: geen actor-id, maar de tijd die iets liet gebeuren. */
const CLOCK_ACTOR = { id: 'klok', label: 'de klok', icon: 'clock' };

/**
 * Wie deze gebeurtenis in gang zette, als iets om te tonen.
 *
 * Drie soorten, en ze horen verschillend te lezen: een **actor** deed iets uit
 * zichzelf, een **cel** deed het als organisatie (ze besloot, of ze vroeg iets
 * aan een ander), en de **klok** deed het omdat de tijd verstreek. Dat verschil
 * weghalen zou een vervallen termijn op iemands naam zetten.
 */
export function journalActor(entry) {
  const actor = entry?.actor;
  if (actor?.soort === 'actor') return { id: actor.id, label: actor.id, icon: 'person' };
  if (actor?.soort === 'cell') return { id: actor.id, label: actor.id, icon: 'building' };
  return CLOCK_ACTOR;
}

/** De regels van het journaal, in de volgorde waarin ze ontstonden. */
export function journalEntries(snapshot) {
  return Array.isArray(snapshot?.journal) ? snapshot.journal : [];
}

/** De actoren die in dit journaal voorkomen, om op te filteren. */
export function journalActors(snapshot) {
  const seen = new Map();
  for (const entry of journalEntries(snapshot)) {
    const actor = journalActor(entry);
    if (!seen.has(actor.id)) seen.set(actor.id, actor);
  }
  return [...seen.values()];
}

/**
 * De cellen waar deze regel over gaat.
 *
 * De cellen waarin een gram landde, plus — bij een cross-cel-vraag — de vrager
 * en de bevraagde. Een filter op cel hoort die vraag te tonen bij beide kanten:
 * zij ging over precies die grens.
 */
export function journalCells(entry) {
  const found = new Set();
  for (const gram of entry?.grams ?? []) found.add(gram.cell);
  for (const change of entry?.changes ?? []) found.add(change.cell);
  const question = entry?.question;
  if (question) {
    found.add(question.answer?.cell);
    // `asked_by` is een identiteit (`cel:<id>`) en geen cel-id; het deel erachter
    // is het cel-id waarop de rest van het beeld draait.
    const asker = askingCell(question);
    if (asker) found.add(asker);
  }
  found.delete(undefined);
  return [...found];
}

/** Het cel-id van de vrager, uit de identiteit die ondertekende. */
export function askingCell(question) {
  const asked = question?.asked_by;
  if (typeof asked !== 'string') return null;
  const index = asked.lastIndexOf(':');
  return index === -1 ? asked : asked.slice(index + 1);
}

/**
 * Is deze regel erbij gekomen sinds het beeld waarop `previousLength` gemeten is?
 *
 * Hetzelfde soort telling als bij de grammen in de kolommen: het journaal groeit
 * achteraan en wijzigt nooit, dus elke regel vanaf het oude aantal is nieuw. Na
 * een reset zakt het aantal en is er niets nieuw — precies goed, want een verse
 * wereld is geen gebeurtenis.
 */
export function isNewEntry(previousLength, entry) {
  if (previousLength === null || previousLength === undefined) return false;
  return entry.seq >= previousLength;
}

/**
 * De regels zoals de weergave ze nodig heeft: gefilterd, gemarkeerd, ingesprongen.
 *
 * Een regel die het filter niet haalt blijft staan met `hidden`, zodat de lijst
 * zelf "niets gevonden" kan zeggen en de filters een weg terug blijven — dezelfde
 * keuze als in het grammenpaneel.
 */
export function journalRows(snapshot, { previousLength = null, actor = '', cell = '', moment = '' } = {}) {
  return journalEntries(snapshot).map((entry) => {
    const who = journalActor(entry);
    const inCells = journalCells(entry);
    const matches =
      (!actor || who.id === actor) &&
      (!cell || inCells.includes(cell)) &&
      (!moment || entry.moment === moment);
    return {
      entry,
      id: `journaal-${entry.seq}`,
      actor: who,
      kind: journalKind(entry.kind),
      cells: inCells,
      grams: journalGrams(snapshot, entry),
      isNew: isNewEntry(previousLength, entry),
      // Een vraag hangt onder het besluit dat haar uitlokte; los gelezen is ze een
      // vraag zonder aanleiding.
      indented: entry.parent !== null && entry.parent !== undefined,
      matches,
    };
  });
}

/**
 * De grammen van één regel, met de zaak waar ze over gaan.
 *
 * De regel draagt alleen een **verwijzing** naar een gram, en dat is de bedoeling
 * — het journaal houdt geen tweede administratie. Het kenmerk komt dus uit het
 * gram zelf, opgezocht in de kroniek waarin het ligt. Een gram zonder zaak levert
 * een lege regel op en geen ontbrekend veld.
 */
export function journalGrams(snapshot, entry) {
  return (entry?.grams ?? []).map((ref) => ({ ...ref, zaak: describeZaak(gramByRef(snapshot, ref)) }));
}

/** De cellen om op te filteren: uit het beeld, ook als er nog niets gebeurde. */
export function journalCellOptions(snapshot) {
  return cells(snapshot).map((cell) => cell.id);
}

/**
 * Eén statusverandering als tekst: `was → is`.
 *
 * Eén uitkomst staat kaal, meer uitkomsten staan met hun naam erbij — dezelfde
 * afweging als in het verslag van de simulator: bij één waarde maakt de naam de
 * regel alleen langer, bij twee is weglaten niet meer te lezen. De **notatie**
 * is wel die van het scherm en niet die van het verslag: elke waarde gaat door
 * `formatValue` (`ja`/`nee`, "nog niet bekend"), en dat hoort hier ook — een
 * verslag wordt gelezen naast de kronieken, dit staat op een pagina. Wie de twee
 * vergelijkt, vergelijkt dus wat er staat en niet hoe het genoteerd is.
 */
export function describeChange(change) {
  return `${describeStand(change?.voor)} → ${describeStand(change?.na)}`;
}

/** Hoe een stand heet; `null` is "niets vastgesteld" en geen lege waarde. */
export function describeStand(stand) {
  if (stand === null || stand === undefined) return 'niets vastgesteld';
  const entries = Object.entries(stand);
  if (entries.length === 0) return 'niets vastgesteld';
  if (entries.length === 1) return formatValue(entries[0][1]);
  return entries.map(([name, value]) => `${name}: ${formatValue(value)}`).join(' · ');
}

/**
 * Eén geaccepteerde waarde als tekst.
 *
 * Dit is invariant I5 in het verhaal: het besluit rekende deze waarde niet na
 * maar haalde hem bij degene die hem vaststelde, en dat hoort in de regel te
 * staan en niet alleen in de herkomst van een veld.
 */
export function describeAccepted(accepted) {
  const value = accepted?.value === undefined || accepted?.value === null ? '' : ` ${formatValue(accepted.value)}`;
  const moment = accepted?.op_moment ? ` (vastgesteld ${accepted.op_moment})` : '';
  return `accepteerde ${accepted?.name}${value} van ${accepted?.cell}${moment}`;
}

/**
 * Het antwoord op een cross-cel-vraag, in één regel.
 *
 * "Niets vastgesteld" is een antwoord en geen leeg antwoord; dat onderscheid is
 * wat deze opstelling maakt, en het hoort hier niet weg te vallen.
 */
export function describeAnswer(question) {
  const outcome = question?.answer?.outcome ?? {};
  if (outcome.established) return describeStand(outcome.established);
  return outcome.not_established?.reason ?? 'niets vastgesteld';
}
