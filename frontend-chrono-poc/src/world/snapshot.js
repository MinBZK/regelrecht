/**
 * Het beeld van de wereld lezen. Niets casus-eigens.
 *
 * Elke naam die een bezoeker ziet — een cel, een kroniek, een actie, een
 * instelling — komt uit het beeld en staat niet hier. Deze module zet dat beeld
 * alleen om in de vorm die de weergave nodig heeft: welke grammen er per cel
 * liggen, waar de tijdlijn punten zet, wie welke actie kan doen en waar de
 * waarde in een decretogram vandaan komt.
 *
 * Het contract is `Snapshot` uit `packages/simulator/src/snapshot.rs`. Waar een
 * veld kan ontbreken (een oudere server, een gram dat zijn herkomst niet kon
 * opschrijven) valt het hier terug op iets leesbaars: een beeld hoort niet om te
 * vallen omdat één herkomst niet te lezen was.
 */
import { formatMoment, formatValue } from './format.js';

/**
 * De drie grammen van de chronolexografie, met hoe ze eruitzien.
 *
 * Het lexogram — de wet zelf — ligt in geen enkele kroniek; het staat hier omdat
 * een cel er wel wetten laadt en een lezer één vocabulaire voor alle drie hoort
 * te hebben.
 */
export const GRAM_KINDS = {
  lexogram: { label: 'Lexogram', color: 'paars', icon: 'book' },
  decretogram: { label: 'Decretogram', color: 'donkerblauw', icon: 'certificate' },
  executogram: { label: 'Executogram', color: 'groen', icon: 'file-text' },
};

const UNKNOWN_KIND = { label: 'Gram', color: 'neutral', icon: 'file' };

/**
 * Het besluittype van een besluit dat op een voorwaarde afketste.
 *
 * Platformvocabulaire en geen casusnaam: het staat zo in het schema
 * (`produces.decision_type`) en de simulator schrijft precies dit woord in het
 * gram. Het staat hier omdat een afwijzing er anders uitziet dan een toekenning
 * — een weigering hoort niet als een gewoon besluit weg te vallen.
 */
const AFWIJZING = 'AFWIJZING';

/**
 * Het veld waaronder een zaak terug te vinden is.
 *
 * Een platformnaam en geen casusnaam: elk decretogram draagt hem, en elke
 * betaling over dezelfde zaak ook (zie `packages/simulator/README.md`, "Wat een
 * decretogram draagt").
 */
const ZAAKKENMERK = 'zaakkenmerk';

/**
 * De stage van de procedure waarin een besluit staat (RFC-008).
 *
 * Platformvocabulaire, zoals de soorten gram: de simulator schrijft deze woorden
 * in het veld `stage`. Ze staan hier omdat de bekendmaking van een besluit een
 * **eigen gram** is op dezelfde zaak — zonder dit label zouden er in een kroniek
 * twee decretogrammen naast elkaar staan die alleen aan hun velden uit elkaar te
 * houden zijn.
 */
export const STAGES = {
  BESLUIT: { label: 'Besluit', color: 'donkerblauw' },
  BEKENDMAKING: { label: 'Bekendmaking', color: 'hemelblauw' },
};

/** Hoe dit gram eruitziet; een onbekend soort blijft leesbaar. */
export function gramKind(kind) {
  return GRAM_KINDS[kind] ?? UNKNOWN_KIND;
}

/**
 * De stage van een gram, als iets om te tonen.
 *
 * `null` als het gram er geen draagt: een executogram hoort bij geen enkele
 * procedure, en dan valt er niets te tonen. Een stage die deze app niet kent,
 * blijft leesbaar met haar eigen naam — welke stages er zijn, zegt de wet en
 * niet deze lijst.
 */
export function stageOf(gram) {
  const value = gram?.fields?.stage?.value;
  if (value === null || value === undefined || value === '') return null;
  const stage = String(value);
  const known = STAGES[stage];
  return { stage, label: known?.label ?? stage, color: known?.color ?? 'neutral' };
}

/**
 * Het besluittype van een gram, als iets om te tonen.
 *
 * `null` als het gram er geen draagt: een executogram is geen besluit, en een
 * decretogram onder een regeling die `produces.decision_type` weglaat draagt
 * `null` — dan valt er niets te tonen en hoort er geen leeg label te staan.
 *
 * De **naam komt uit het beeld** en niet uit een lijst hier: welke besluittypen
 * er zijn, is aan de regeling. Alleen de afwijzing krijgt een eigen kleur, want
 * dat is het ene onderscheid dat het platform zelf maakt.
 */
export function decisionTypeOf(gram) {
  const value = gram?.fields?.decision_type?.value;
  if (value === null || value === undefined || value === '') return null;
  const type = String(value);
  return { type, label: type, color: type === AFWIJZING ? 'oranje' : 'donkerblauw' };
}

/**
 * Waarop een besluit afketste, als rijen.
 *
 * Elke grond noemt de uitkomst, de waarde die tot afwijzing leidde en het
 * artikel dat de uitkomst voortbrengt. Leeg bij elk besluit dat niet afwees —
 * en dat is het gewone geval, dus een lege lijst is geen ontbrekend veld.
 */
export function afwijzingsgrondenOf(gram) {
  return readAfwijzingsgronden(gram?.fields?.afwijzingsgrond?.value);
}

/**
 * De afwijzingsgronden uit de ruwe waarde van het veld `afwijzingsgrond`.
 *
 * Los van het gram, want een lexostatus die het veld publiceert geeft dezelfde
 * lijst als kale waarde terug. De vorm is `Afwijzingsgrond` uit
 * `packages/simulator/src/cell/besluit.rs`: `output`, `value` en `article`.
 */
function readAfwijzingsgronden(raw) {
  if (!Array.isArray(raw)) return [];
  return raw.map((row) => ({
    output: row?.output === undefined || row?.output === null ? null : String(row.output),
    value: row?.value,
    article: row?.article === undefined || row?.article === null ? null : String(row.article),
  }));
}

/**
 * De uitkomsten die de eigen regeling voor deze stage declareerde en die niet
 * kwamen, als rijen.
 *
 * Elke rij noemt de uitkomst, het artikel met regeling en versie, en de reden
 * als de engine die gaf. Leeg bij een gram waarin elke gedeclareerde uitkomst er
 * kwam, en bij elk gram dat geen bekendmaking is — het veld staat er dan niet.
 * Zonder deze rijen is een uitkomst die niet gedeclareerd was in beeld niet te
 * onderscheiden van een die gedeclareerd was en ontbrak.
 */
export function stageUitkomstenNietGeleverdOf(gram) {
  const raw = gram?.fields?.stage_uitkomst_niet_geleverd?.value;
  if (!Array.isArray(raw)) return [];
  return raw.map((row) => ({
    uitkomst: row?.uitkomst === undefined || row?.uitkomst === null ? null : String(row.uitkomst),
    lexogram: describeLexogram(
      row?.lexogram
        ? {
            regulation: row.lexogram.regulation,
            article: row.lexogram.artikel,
            valid_from: row.lexogram.regulation_valid_from,
          }
        : null,
    ),
    reden: row?.reden === undefined || row?.reden === null ? null : String(row.reden),
  }));
}

/** De cellen uit het beeld, in de volgorde waarin het beeld ze geeft. */
export function cells(snapshot) {
  return Array.isArray(snapshot?.cells) ? snapshot.cells : [];
}

/** De kronieken van een cel, elk met zijn grammen. */
export function chronicles(cell) {
  return Array.isArray(cell?.chronicles) ? cell.chronicles : [];
}

/**
 * De lexostatussen die een cel publiceert, zoals zij ze documenteert.
 *
 * Elk item draagt `name`, `doc`, de `inputs` met hun type, de `outputs` en — bij
 * een kroniekfilter — de `key`: de stroom en het sleutelveld waarop gereduceerd
 * wordt. Deze app verzint er niets bij; het staat allemaal in het beeld.
 */
export function lexostatusDefinitions(cell) {
  return Array.isArray(cell?.lexostatussen) ? cell.lexostatussen : [];
}

/** De besluiten die een cel kan nemen, met hun zaakkenmerk-sjabloon. */
export function besluitDefinitions(cell) {
  return Array.isArray(cell?.besluiten) ? cell.besluiten : [];
}

/**
 * De vier herkomsten die een veld van een decretogram kan hebben.
 *
 * Uit `packages/simulator/src/cell/schema.rs`; deze app verzint er geen vijfde
 * bij. Het **gat** is de reden dat dit schema bestaat: normatieve inhoud die in
 * het wereldbestand staat in plaats van in de wet (RFC-022), en die hoort er
 * anders uit te zien dan een veld dat de wet wél declareert.
 */
export const FIELD_SOURCES = {
  lexogram: { label: 'wet', color: 'paars', icon: 'book' },
  beleid: { label: 'beleid', color: 'oranje', icon: 'book' },
  wereldbestand: { label: 'wereldbestand', color: 'rood', icon: 'warning' },
  platform: { label: 'platform', color: 'neutral', icon: 'server' },
};

const UNKNOWN_SOURCE = { label: 'onbekend', color: 'neutral', icon: 'question' };

/**
 * Het schema van het decretogram dat een besluit kan voortbrengen, klaar om te
 * tonen.
 *
 * Per veld: de naam, het type met zijn eenheid, het herkomst-label en — als een
 * regeling het veld declareert — welk artikel van welke versie dat is. Wat deze
 * functie *niet* doet, is een herkomst afleiden: het beeld zegt het, en een veld
 * waarvan het beeld niets zegt, blijft leesbaar in plaats van te verdwijnen.
 */
export function decretogramSchema(besluit) {
  return (Array.isArray(besluit?.schema) ? besluit.schema : []).map((field) => ({
    name: field.name,
    type: [field.type ?? 'onbekend', field.unit].filter(Boolean).join(' · '),
    gat: Boolean(field.gat),
    source: FIELD_SOURCES[field.herkomst] ?? UNKNOWN_SOURCE,
    lexogram: describeLexogram(field.lexogram),
    toelichting: field.toelichting ?? '',
  }));
}

/**
 * Het artikel waar een veld vandaan komt, als één regel; leeg als geen regeling
 * het declareert.
 *
 * De **versie** hoort erbij: een verwijzing naar een artikel zonder de versie
 * waarin het zo luidt, wijst naar iets dat morgen anders kan staan.
 */
function describeLexogram(lexogram) {
  if (!lexogram) return '';
  const plek = lexogram.article ? `artikel ${lexogram.article}` : 'op het document';
  const versie = lexogram.valid_from ? ` (versie ${formatMoment(lexogram.valid_from)})` : '';
  return `${lexogram.regulation}, ${plek}${versie}`;
}

/**
 * De parameters van één lexostatus, met wat een vrager erover hoort te weten.
 *
 * Een parameter is meestal een waarde die de vrager al heeft (een BSN). Maar de
 * **sleutel** van een kroniekfilter is iets anders: dat is waaronder deze cel
 * haar vastleggingen groepeert, en bij een kroniek met beschikkingen is dat het
 * zaakkenmerk — een samengestelde waarde die uit het sjabloon van een besluit
 * ontstaat. Wie dat niet weet, typt daar wat en krijgt "niets vastgesteld"
 * terug: een geldig antwoord op een vraag over een zaak die niet bestaat.
 *
 * Daarom komt er bij zo'n parameter drie dingen bij: de kroniek waarvan ze de
 * sleutel is, de **vorm** die de besluiten van deze cel eraan geven, en de
 * waarden die er nu in die kroniek onder liggen.
 */
export function lexostatusParams(cell, definition) {
  const key = definition?.key ?? null;
  return (definition?.inputs ?? []).map((input) => {
    const isKey = Boolean(key) && key.parameter === input.name;
    return {
      name: input.name,
      type: input.type,
      chronicle: isKey ? key.chronicle : null,
      patterns: isKey ? zaakkenmerkPatterns(cell, key.chronicle) : [],
      known: isKey ? knownKeys(cell, key.chronicle, key.parameter) : [],
    };
  });
}

/**
 * Wat er onder het label van zo'n parameter staat.
 *
 * Bij een sleutel de kroniek en de vorm, zodat te lezen is waar de waarde
 * vandaan komt; anders het woord dat de wereld voor het type gebruikt.
 */
export function describeParam(param) {
  if (!param?.chronicle) return String(param?.type ?? '');
  const vorm = param.patterns.length > 0 ? `, vorm ${param.patterns.map((text) => `'${text}'`).join(' of ')}` : '';
  return `sleutel van kroniek '${param.chronicle}'${vorm}`;
}

/**
 * Het voorbeeld dat als geestschrift in het invoerveld staat; leeg als er geen
 * eenduidig voorbeeld te geven is.
 *
 * Een placeholder staat op de plek waar de invuller zo zelf iets neerzet, dus
 * het hoort één **welgevormde** waarde te zijn. Bij precies één vorm is dat die
 * vorm. Zijn er meer — twee besluiten met elk een eigen sjabloon in dezelfde
 * kroniek — dan is er geen voorbeeld dat de andere niet tegenspreekt: er één
 * uitkiezen laat de vorm stelliger lijken dan ze is, en ze aaneenrijgen zet een
 * tekst in het veld die zelf geen geldige waarde is. Dan blijft het veld leeg en
 * doet [`describeParam`] het werk: die somt ze alle op in het label erboven, en
 * dat is de plek voor een opsomming.
 */
export function placeholderFor(param) {
  const patterns = param?.patterns ?? [];
  return patterns.length === 1 ? patterns[0] : '';
}

/**
 * De zaakkenmerk-sjablonen van de besluiten die in deze kroniek leggen.
 *
 * Uit de besluiten van deze cel en niet uit een lijst hier: welke vorm een
 * zaakkenmerk heeft, staat in het wereldbestand. Een kroniek waarin geen besluit
 * van deze cel legt — de betalingen, die het kenmerk van een ander dragen —
 * levert niets op, en dan staat er ook geen vorm bij.
 */
function zaakkenmerkPatterns(cell, chronicle) {
  const seen = [];
  for (const besluit of besluitDefinitions(cell)) {
    if (besluit.chronicle !== chronicle) continue;
    if (besluit.zaakkenmerk && !seen.includes(besluit.zaakkenmerk)) seen.push(besluit.zaakkenmerk);
  }
  return seen;
}

/**
 * De waarden die nu in deze kroniek onder dit veld liggen.
 *
 * Wat de cel al heeft vastgelegd, dus: geen lijst die deze app bijhoudt, maar
 * een doorsnede van het beeld dat er toch al is. Een gram dat ná de klok ligt
 * telt gewoon mee — dat de vraag erover "niets vastgesteld" oplevert, is een
 * antwoord over het gekozen moment en geen reden om de waarde te verzwijgen.
 */
function knownKeys(cell, chronicle, field) {
  const stream = chronicles(cell).find((candidate) => candidate.stream === chronicle);
  const seen = [];
  for (const gram of stream?.grams ?? []) {
    const value = gram?.fields?.[field]?.value;
    if (value === null || value === undefined || value === '') continue;
    const text = String(value);
    if (!seen.includes(text)) seen.push(text);
  }
  return seen.sort((a, b) => a.localeCompare(b));
}

/**
 * Het zaakkenmerk dat een gram draagt; `null` als het er geen heeft.
 *
 * Uit het veld en niet uit de soort van het gram: een besluit draagt het als
 * vast veld, en een betaling die over diezelfde zaak gaat draagt het ook. Dat
 * beide het dragen, is precies hoe ze bij elkaar te vinden zijn.
 */
export function zaakkenmerkOf(gram) {
  const value = gram?.fields?.[ZAAKKENMERK]?.value;
  return value === null || value === undefined || value === '' ? null : String(value);
}

/** De zaak van een gram als regel; leeg als het gram er geen draagt. */
export function describeZaak(gram) {
  const kenmerk = zaakkenmerkOf(gram);
  return kenmerk ? `zaak ${kenmerk}` : '';
}

/**
 * Het id waarmee het beeld één gram aanwijst: `<cel>|<kroniek>|<plek>`.
 *
 * Eén plek voor die vorm, want hij wordt op drie manieren gebruikt — het beeld
 * bouwt hem per rij, een betaling draagt hem als losse velden, en [`gramByRef`]
 * leest hem weer uit elkaar. Drie kopieën van dezelfde afspraak zouden alle drie
 * apart moeten meeverhuizen als de vorm ooit verandert.
 */
export function gramId(cell, chronicle, index) {
  return `${cell}|${chronicle}|${index}`;
}

/**
 * Is dit een plek zoals een gram-id haar draagt?
 *
 * Op de tekst getoetst en niet op wat `Number` ervan maakt: `Number('')` is 0 en
 * `Number(' 1 ')` is 1, dus een lege of slordige plek zou anders stil een gram
 * áánwijzen — een verwijzing die klopt lijkt te zijn terwijl ze het niet is.
 * Niets teruggeven is hier het eerlijke antwoord.
 */
function isPlace(position) {
  return /^\d+$/.test(position);
}

/**
 * Het gram waar een verwijzing naar wijst, uit de kroniek waarin het ligt.
 *
 * Het id is `<cel>|<kroniek>|<plek>`, en die plek is de index in precies de
 * lijst die het beeld geeft: een kroniek groeit achteraan en wijzigt nooit. Een
 * verwijzing draagt daarom geen kopie van het gram, en dit is de weg terug.
 */
export function gramByRef(snapshot, ref) {
  const parts = String(ref?.id ?? '').split('|');
  if (parts.length !== 3) return null;
  const [cellId, stream, position] = parts;
  if (!isPlace(position)) return null;
  const index = Number(position);
  const cell = cells(snapshot).find((candidate) => candidate.id === cellId);
  const chronicle = chronicles(cell).find((candidate) => candidate.stream === stream);
  return (chronicle?.grams ?? [])[index] ?? null;
}

/**
 * De grammen van een kroniek in tijdsvolgorde, elk met zijn plek in het beeld.
 *
 * De dag is de korrel, dus bij een gelijk moment blijft de volgorde van
 * vastlegging staan. `index` is die plek in het beeld en reist mee, want daaraan
 * hangt of een gram nieuw is (zie [`isNewGram`]).
 */
export function gramsInTimeOrder(chronicle) {
  const grams = Array.isArray(chronicle?.grams) ? chronicle.grams : [];
  return grams
    .map((gram, index) => ({ gram, index }))
    .sort((a, b) => String(a.gram.op_moment).localeCompare(String(b.gram.op_moment)) || a.index - b.index);
}

/**
 * Hoeveel grammen er per kroniek liggen: `'cel|stroom' -> aantal`.
 *
 * Waarmee "nieuw" te bepalen is zonder de server ernaar te vragen. Een cel legt
 * een gram achteraan in zijn kroniek en wijzigt nooit een bestaand gram, dus
 * elk gram vanaf het oude aantal is erbij gekomen. Na een reset zakt het aantal
 * en is er niets nieuw — precies goed, want een startstand is geen gebeurtenis.
 */
export function gramCounts(snapshot) {
  const counts = new Map();
  for (const cell of cells(snapshot)) {
    for (const chronicle of chronicles(cell)) {
      counts.set(streamKey(cell.id, chronicle.stream), (chronicle.grams ?? []).length);
    }
  }
  return counts;
}

/** De sleutel waaronder één kroniekstroom in `gramCounts` staat. */
export function streamKey(cellId, stream) {
  return `${cellId}|${stream}`;
}

/**
 * Is dit gram erbij gekomen sinds het beeld waarop `counts` gemaakt is?
 *
 * `index` is de plek in de kroniek zoals het beeld haar geeft (vastlegorde).
 */
export function isNewGram(counts, cellId, stream, index) {
  if (!counts) return false;
  const before = counts.get(streamKey(cellId, stream));
  if (before === undefined) return true;
  return index >= before;
}

/**
 * De grammen die erbij kwamen sinds het beeld waarop `counts` gemaakt is.
 *
 * Waarmee een uitgevoerde actie te verslaan is in de woorden van de wereld: dit
 * gram, in die kroniek van die cel, op dat moment.
 */
export function newGrams(snapshot, counts) {
  const added = [];
  if (!counts) return added;
  for (const cell of cells(snapshot)) {
    for (const chronicle of chronicles(cell)) {
      (chronicle.grams ?? []).forEach((gram, index) => {
        if (!isNewGram(counts, cell.id, chronicle.stream, index)) return;
        added.push({
          cell: cell.id,
          stream: chronicle.stream,
          kind: gram.kind,
          name: gram.name,
          opMoment: gram.op_moment,
        });
      });
    }
  }
  return added;
}

/**
 * De punten van de tijdlijn: elk moment waarop er iets ligt, met de klok erin.
 *
 * Eén punt per dag, want de dag is de korrel van deze wereld. Het punt van de
 * klok staat er altijd, ook als er die dag niets ligt: waar de klok staat is
 * zelf iets om te zien. Wat vóór de klok ligt is `past`, wat erna komt `future`.
 */
export function timelineMoments(snapshot, { newGramCounts } = {}) {
  const clock = snapshot?.clock ?? null;
  const byMoment = new Map();
  const at = (moment) => {
    if (!byMoment.has(moment)) {
      byMoment.set(moment, { moment, grams: [], hasNew: false, isClock: moment === clock });
    }
    return byMoment.get(moment);
  };
  if (clock) at(clock);

  for (const cell of cells(snapshot)) {
    for (const chronicle of chronicles(cell)) {
      (chronicle.grams ?? []).forEach((gram, index) => {
        const point = at(gram.op_moment);
        const isNew = isNewGram(newGramCounts, cell.id, chronicle.stream, index);
        point.grams.push({
          cell: cell.id,
          stream: chronicle.stream,
          kind: gram.kind,
          name: gram.name,
          isNew,
        });
        if (isNew) point.hasNew = true;
      });
    }
  }

  return [...byMoment.values()]
    .sort((a, b) => String(a.moment).localeCompare(String(b.moment)))
    .map((point) => ({
      ...point,
      step: clock === null || point.moment === clock ? 'current' : point.moment < clock ? 'past' : 'future',
    }));
}

/** Het nummer (1-gebaseerd) van het punt waar de klok staat; 1 als er geen is. */
export function clockIndex(moments) {
  const index = moments.findIndex((point) => point.isClock);
  return index === -1 ? 1 : index + 1;
}

/**
 * De acties uit het beeld, gegroepeerd per actor.
 *
 * Een actie die nu niet kan blijft staan, met de reden erbij: wie alleen het
 * mogelijke toont, laat een keuze verdwijnen zonder te zeggen waarom.
 */
export function actionsByActor(snapshot) {
  const groups = new Map();
  for (const action of Array.isArray(snapshot?.actions) ? snapshot.actions : []) {
    const actor = action.actor ?? '';
    if (!groups.has(actor)) groups.set(actor, { actor, actions: [] });
    groups.get(actor).actions.push(action);
  }
  return [...groups.values()];
}

/**
 * Het beginformulier van een actie: elk veld uit `form`, voorgevuld waar het
 * beeld iets aanlevert en anders leeg naar zijn type.
 *
 * De voorinvulling is al opgelost door de wereld (`action.prefill`): daar staat
 * een waarde en geen verwijzing, dus deze app zoekt niets op in de kronieken.
 * Wat er niet in staat, begint leeg — een veld waarover niets bekend is, hoort
 * niet met `null` gevuld te worden, want dat is in een formulier een ingevulde
 * afwezigheid.
 *
 * `typed` houdt de velden die de invuller zelf invulde: die zijn van hem en
 * blijven staan, ook als de wereld intussen iets anders voorstelt. De rest volgt
 * het nieuwe beeld, en dat is het punt — de klok loopt door en er komen feiten
 * bij, dus een veld dat niemand aanraakte hoort te zeggen wat de wereld *nu* al
 * weet en niet wat zij een paar dagen geleden wist.
 */
export function initialForm(action, typed = {}) {
  const values = {};
  const prefill = prefillOf(action);
  for (const field of Array.isArray(action?.form) ? action.form : []) {
    if (field.name in typed) {
      values[field.name] = typed[field.name];
      continue;
    }
    const suggested = prefill[field.name];
    values[field.name] = suggested === undefined || suggested === null ? emptyValue(field.type) : suggested;
  }
  return values;
}

/**
 * Hoe een leeg veld van dit type eruitziet.
 *
 * `amount` telt hier als getal: een bedrag is er een, en een leeg bedrag is dus
 * niets en geen lege tekst — anders zou een niet-ingevuld bedrag als `''` de
 * deur uit gaan.
 */
function emptyValue(type) {
  if (type === 'number' || type === 'amount') return null;
  if (type === 'boolean') return false;
  return '';
}

/** De voorinvulling die het beeld bij deze actie geeft; leeg als ze er niet is. */
function prefillOf(action) {
  const prefill = action?.prefill;
  return prefill && typeof prefill === 'object' ? prefill : {};
}

/**
 * Staat dit veld nog op wat de wereld voorstelde?
 *
 * Zodra iemand er iets anders van maakt, is het zijn opgave en niet meer een
 * voorinvulling — dus dan hoort het label dat ook niet meer te zeggen.
 */
export function isPrefilled(action, name, value) {
  const prefill = prefillOf(action);
  return name in prefill && prefill[name] === value;
}

/**
 * Het besluit dat er al ligt voor de zaak die dit formulier aanwijst.
 *
 * Een `decides`-actie mag twee keer: een tweede besluit over dezelfde zaak is
 * het verhaal en geen vergissing (een toekenning en daarna een vaststelling, of
 * een herzien inkomen). Wat er niet hoort te gebeuren, is dat iemand het
 * *onbedoeld* doet — twee keer klikken leverde twee decretogrammen zonder dat er
 * iets over gezegd werd. Daarom geeft deze functie terug wat er al ligt, zodat
 * de kaart het kan tonen en om bevestiging kan vragen.
 *
 * "Dezelfde zaak" leest deze app uit de parameters van het besluit en niet uit
 * het zaakkenmerk-sjabloon. Dat sjabloon staat wel in het beeld — het is te zien
 * bij de sleutel van een kroniek — maar het **invullen** ervan is werk van de
 * cel, met een weigering eraan vast voor een waarde waarin een scheidingsteken
 * voorkomt. Een tweede plek die het kenmerk samenstelt zou daarvan af kunnen
 * wijken en dan een zaak aanwijzen die de cel niet bedoelt. Een decretogram legt
 * elke parameter vast onder haar eigen naam, dus het gram waarvan alle parameters
 * overeenkomen met wat er in het formulier staat, gaat over dezelfde zaak. Het
 * **zaakkenmerk** van dat gram komt mee, want dat is waaronder de zaak terug te
 * vinden is.
 *
 * `null` zolang er niets te melden is: geen besluit-actie, een formulier dat nog
 * niet ingevuld is, of geen gram dat erbij past. Grammen ná de klok tellen niet
 * mee; de vraag is wat er nú al ligt.
 */
export function decidedAlready(snapshot, action, values) {
  if (action?.effect?.soort !== 'decides') return null;
  const params = (action.form ?? []).map((field) => field.name);
  const given = params.map((param) => values?.[param]);
  if (given.some((value) => value === '' || value === null || value === undefined)) return null;

  const clock = snapshot?.clock ?? null;
  const cell = cells(snapshot).find((candidate) => candidate.id === action.effect.cell);
  const matches = [];
  for (const chronicle of chronicles(cell)) {
    for (const gram of chronicle.grams ?? []) {
      if (gram.kind !== 'decretogram') continue;
      if (gram.fields?.besluit?.value !== action.effect.besluit) continue;
      if (clock && String(gram.op_moment) > clock) continue;
      const sameCase = params.every(
        (param, index) => String(gram.fields?.[param]?.value ?? '') === String(given[index]),
      );
      if (sameCase) matches.push(gram);
    }
  }
  if (matches.length === 0) return null;

  // Het laatste besluit is wat een lezer herkent; hoeveel er liggen zegt wat een
  // volgende erbij doet.
  const latest = matches.reduce((a, b) => (String(a.op_moment) > String(b.op_moment) ? a : b));
  return {
    opMoment: latest.op_moment,
    zaakkenmerk: zaakkenmerkOf(latest),
    count: matches.length,
  };
}

/**
 * Wat een actie uitwerkt, in één regel.
 *
 * Uit het beeld, dus in de woorden van de wereld: welke cel legt vast in welke
 * kroniek, en aan wie wordt hetzelfde feit geleverd — of welk besluit gaat lopen.
 */
export function describeEffect(effect) {
  if (effect?.soort === 'records') {
    const delivered = effect.delivers_to ? `, geleverd aan cel '${effect.delivers_to}'` : '';
    return `legt '${effect.name}' vast in kroniek '${effect.chronicle}' van cel '${effect.cell}'${delivered}`;
  }
  if (effect?.soort === 'decides') {
    return `laat cel '${effect.cell}' het besluit '${effect.besluit}' nemen`;
  }
  if (effect?.soort === 'publishes') {
    return `laat cel '${effect.cell}' het besluit '${effect.besluit}' bekendmaken`;
  }
  return '';
}

/**
 * De velden van een gram, met hun herkomst, op naam gesorteerd.
 *
 * Het **zaakkenmerk** staat vooraan, daarna de overige vaste velden van een
 * besluit (regeling, bevoegd gezag), daarna de rest. Het kenmerk voorop omdat
 * het zegt waar dit gram bij hoort: elke andere waarde erin gaat over die ene
 * zaak, en wie van onder naar boven leest weet pas aan het eind welke.
 */
export function gramFields(gram) {
  const entries = Object.entries(gram?.fields ?? {});
  const rank = (field) => {
    if (field.name === ZAAKKENMERK) return 0;
    return field.origin?.herkomst === 'besluit' ? 1 : 2;
  };
  return entries
    .map(([name, field]) => ({ name, value: field?.value, origin: field?.origin ?? null }))
    .sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
}

/**
 * Waar één waarde vandaan komt, als iets om te tonen.
 *
 * `label` is het korte antwoord, `details` de onderbouwing als paren. Voor een
 * geaccepteerde waarde staat daar de cel, de lexostatus, het moment en de
 * ondertekening in: dat is invariant I5 in beeld — geaccepteerd hoort niet op
 * berekend te lijken.
 */
export function describeOrigin(origin) {
  switch (origin?.herkomst) {
    case 'recorded':
      return {
        kind: 'recorded',
        label: 'vastgelegd door de cel zelf',
        color: 'neutral',
        details: pairs([
          ['kanaal', origin.intake],
          ['grondslag', origin.grondslag],
        ]),
      };
    case 'besluit':
      return {
        kind: 'besluit',
        label: 'vast veld van het besluit',
        color: 'neutral',
        details: [],
      };
    case 'computed':
      return {
        kind: 'computed',
        label: 'berekend door de regeling',
        color: 'donkerblauw',
        details: pairs([['regeling', origin.regulation]]),
      };
    case 'besluit_input':
      return describeRecordedOrigin(origin.recorded_origin);
    default:
      return { kind: 'onbekend', label: 'herkomst niet vastgelegd', color: 'neutral', details: [] };
  }
}

/**
 * De herkomst van een besluit-input, zoals het gram haar opschreef.
 *
 * Geëxporteerd omdat het journaal dezelfde herkomst toont bij de inputs van een
 * besluit (`Execution` uit `packages/simulator/src/journal.rs`): het gram en het
 * verhaal horen over dezelfde waarde niet verschillende woorden te gebruiken.
 */
export function describeRecordedOrigin(recorded) {
  switch (recorded?.herkomst) {
    case 'geaccepteerd':
      return {
        kind: 'geaccepteerd',
        label: `geaccepteerd van cel '${recorded.cell}'`,
        color: 'oranje',
        details: pairs([
          // Het gezag dat de bron-cel bij haar antwoord noemde. Een cel-id is een
          // adres; wie het gram terugleest, hoort te zien wiens vaststelling dit
          // is. Zwijgt de bron erover, dan staat er niets — geen ingevuld gat.
          ['bevoegd gezag', recorded.competent_authority],
          ['lexostatus', recorded.lexostatus],
          ['uitkomst', recorded.field],
          ['op moment', recorded.op_moment],
          ['gevraagd door', recorded.asked_by],
          ['ondertekend', recorded.signature],
          // Welke vraag van dit besluit het antwoord leverde. Lezen meerdere
          // inputs uit één antwoord, dan staat hier bij elk hetzelfde nummer.
          ['contact', recorded.contact],
        ]),
      };
    case 'eigen_kroniek':
      return {
        kind: 'eigen_kroniek',
        label: `uit de eigen kroniek '${recorded.chronicle}'`,
        color: 'groen',
        details: pairs([
          ['veld', recorded.field],
          ['vastgelegd op', recorded.op_moment],
        ]),
      };
    case 'parameter':
      return {
        kind: 'parameter',
        label: 'opgave bij de actie',
        color: 'lintblauw',
        details: pairs([['parameter', recorded.parameter]]),
      };
    case 'eerder_besluit':
      // Een eigen label en niet dat van de eigen kroniek: hier is een *besluit*
      // teruggelezen, en dat is iets anders dan een feit dat de cel overkwam. Het
      // moment erbij, want het bedrag komt uit het gram zoals het toen vastgelegd
      // is — niet uit een herberekening van nu.
      return {
        kind: 'eerder_besluit',
        label: `uit eerder besluit '${recorded.besluit}' over deze zaak (${formatMoment(recorded.moment)})`,
        color: 'paars',
        details: pairs([
          ['zaak', recorded.zaakkenmerk],
          ['besloten op', formatMoment(recorded.moment)],
        ]),
      };
    case 'besluit_uitkomst':
    case 'besluit_input':
      // Een waarde die een hook op een latere stage uit het gram van hetzelfde
      // besluit las. Geen eerder besluit: het is dit besluit, een stap verder in
      // zijn procedure. De laag van het gram zegt of het een uitkomst of een
      // input van het besluit was.
      return {
        kind: recorded.herkomst,
        label: `${recorded.herkomst === 'besluit_uitkomst' ? 'uitkomst' : 'input'} van het besluit '${recorded.besluit}'`,
        color: 'donkerblauw',
        details: pairs([
          ['veld', recorded.field],
          ['gram', recorded.besluit_gram],
        ]),
      };
    default:
      return { kind: 'onbekend', label: 'input, herkomst niet te lezen', color: 'neutral', details: [] };
  }
}

/** Paren zonder de lege: een ontbrekende grondslag is geen regel in beeld. */
function pairs(entries) {
  return entries
    .filter(([, value]) => value !== null && value !== undefined && value !== '')
    .map(([term, value]) => ({ term, value: String(value) }));
}

/**
 * Wie er volgens dit gram mocht besluiten, en wie er besloot.
 *
 * `authority` is wat de **regeling** aanwijst (RFC-002) en `decidedBy` de
 * identiteit van de cel die besloot. Ze zijn gelijk zodra er een gezag is — een
 * besluit door iemand anders wordt geweigerd — dus het geval dat er iets te
 * tonen valt, is juist `authority: null`: dan declareert de regeling geen
 * bevoegd gezag en viel er niets te toetsen. `null` voor het geheel betekent dat
 * dit gram helemaal geen besluit is.
 */
export function competentAuthorityOf(gram) {
  const field = gram?.fields?.competent_authority;
  if (!field) return null;
  const authority = field.value === null || field.value === undefined ? null : String(field.value);
  const decidedBy = gram.fields.besloten_door?.value;
  return { authority, decidedBy: decidedBy === undefined ? null : String(decidedBy) };
}

/**
 * De regeling waaronder een decretogram genomen is, met haar versie.
 *
 * Uit de vaste velden van het gram; een bron-cel die zelf iets vaststelt heeft
 * ze niet, en dan is er niets te tonen (`null`).
 */
export function regulationOf(gram) {
  const regulation = gram?.fields?.regulation?.value;
  if (!regulation) return null;
  return { regulation: String(regulation), validFrom: gram.fields.regulation_valid_from?.value ?? null };
}

/**
 * Het besluit waaruit een executogram over een betaling volgt.
 *
 * Een betaling — en de melding ervan bij de cel die besloot — draagt de
 * verwijzing naar het decretogram als losse velden: de cel, haar kroniek, de
 * plek van het gram daarin, en het termijnnummer. Hier komen die samen tot het
 * id waarmee het beeld een gram aanwijst (`<cel>|<kroniek>|<plek>`), zodat een
 * lezer van de betaling bij het besluit — en dus bij de uitvoeringstrace in zijn
 * receipt — kan komen zonder de kroniek van de besluitende cel af te lopen.
 *
 * `null` als het gram geen betaling is of de verwijzing niet compleet draagt.
 * Half tonen zou een link opleveren die soms nergens op uitkomt, en dat is
 * erger dan geen link.
 */
export function decretogramRefOf(gram) {
  const fields = gram?.fields;
  const text = (name) => {
    const value = fields?.[name]?.value;
    return value === null || value === undefined || value === '' ? null : String(value);
  };
  const cell = text('besluit_cel');
  const chronicle = text('besluit_kroniek');
  const place = text('besluit_gram');
  if (!cell || !chronicle || place === null || !isPlace(place)) return null;
  const volgnummer = text('volgnummer');
  return {
    id: gramId(cell, chronicle, place),
    cell,
    chronicle,
    index: Number(place),
    besluit: text('besluit'),
    opMoment: text('besluit_op_moment'),
    volgnummer: volgnummer === null ? null : Number(volgnummer),
  };
}

/**
 * De kolom waarin een termijn de cel noemt die haar nakomt.
 *
 * De enige naam die deze module van een verplichting kent, en alleen omdat een
 * **afwezigheid** er iets betekent: zie [`obligationValue`].
 */
const OBLIGATION_PAYER = 'betaler';

/**
 * De verplichtingen die uit een besluit volgen, als rijen.
 *
 * Elke verplichting draagt haar eigen namen (soort, schuldenaar, schuldeiser,
 * bedrag, vervaldatum, betaler); de kolommen komen daarom uit de verplichtingen
 * zelf en niet uit een lijst hier.
 */
export function obligationsOf(gram) {
  const raw = gram?.fields?.obligations?.value;
  if (!Array.isArray(raw)) return { columns: [], rows: [] };
  const columns = [];
  for (const row of raw) {
    for (const name of Object.keys(row ?? {})) if (!columns.includes(name)) columns.push(name);
  }
  return { columns, rows: raw };
}

/**
 * Eén waarde uit een verplichtingsregel, als tekst.
 *
 * Als [`formatValue`], op één kolom na. Een verplichting noemt twee partijen met
 * een naam uit het recht; welke **cel** de schuldenaar in deze wereld nakomt, is
 * een andere vraag, en het antwoord mag "niemand" zijn. Dat is geen gat in het
 * gram: de verplichting staat er, de termijn is ingeroosterd, en er is alleen
 * niemand in deze wereld die haar nakomt. 'geen' zou dat een ontbrekende waarde
 * noemen, en dan leest een openstaande termijn als een fout.
 */
export function obligationValue(column, value) {
  if (column === OBLIGATION_PAYER && (value === null || value === undefined)) {
    return 'openstaand — deze wereld kent geen cel voor de schuldenaar';
  }
  return formatValue(value);
}

/**
 * Het antwoord op een lexostatus-vraag, uitgesplitst.
 *
 * "Niets vastgesteld" is een antwoord en geen fout: het komt hier als
 * `established: false` met de reden die de cel gaf.
 */
export function readLexostatus(answer) {
  const outcome = answer?.outcome ?? {};
  const shared = {
    cell: answer?.cell ?? null,
    name: answer?.name ?? null,
    opMoment: answer?.op_moment ?? null,
    reductie: readReductie(answer?.reductie),
  };
  if (outcome.established) {
    return {
      ...shared,
      established: true,
      values: Object.entries(outcome.established).map(([key, value]) => ({ name: key, value })),
      reason: null,
    };
  }
  return {
    ...shared,
    established: false,
    values: [],
    reason: outcome.not_established?.reason ?? 'In deze cel is hierover niets vastgesteld.',
  };
}

/**
 * De namen die een besluit van deze cel als **uitkomst** vastlegt: wat de
 * regeling uitrekende, en niet wat het gram over het besluit zelf zegt.
 *
 * Uit het beeld (`besluiten[].outputs`) en niet uit een lijst hier: welke velden
 * een decretogram vast draagt, weet het platform, en een kopie van die namen
 * loopt stil achter zodra er een bij komt. Wat geen uitkomst is, is dan vanzelf
 * een vast veld — ook een dat deze app nog nooit zag.
 *
 * Noemt het antwoord zijn besluit (`besluit`) en kent de cel dat, dan de
 * uitkomsten van dát besluit. Anders die van alle besluiten van de cel samen:
 * een uitkomstnaam kan niet ook een vast veld zijn (het optuigen weigert dat),
 * dus de vereniging maakt van geen enkel vast veld een uitkomst.
 */
function besluitOutputs(cell, answeredBesluit) {
  const definitions = besluitDefinitions(cell);
  const answered = definitions.filter((definition) => definition.name === answeredBesluit);
  const chosen = answered.length > 0 ? answered : definitions;
  return new Set(chosen.flatMap((definition) => (Array.isArray(definition.outputs) ? definition.outputs : [])));
}

/**
 * Een antwoord dat een afwijzing publiceert, uitgesplitst voor de weergave.
 *
 * `null` als het antwoord geen `decision_type` publiceert, of een ander type dan
 * `AFWIJZING`: dan is er niets uit te splitsen en blijft het antwoord zoals het
 * is. Anders drie delen:
 *
 * - `gronden`: waarop het besluit afketste, zoals het gram ze draagt;
 * - `berekend`: de uitkomsten die de regeling wél uitrekende — een afwijzing
 *   zet die niet op nul, ze belooft er alleen niets mee. De uitkomst die een
 *   grond noemt, staat al bij die grond en niet nog eens hier;
 * - `vast`: de rest, de vaste velden van het gram (`besluit`,
 *   `competent_authority`, ...), die over het besluit gaan en niet over een
 *   bedrag. Zonder `afwijzingsgrond`, want die staat al bovenaan.
 *
 * Wat een uitkomst is, zegt `cell` uit het beeld (zie `besluitOutputs`). Zonder
 * cel is er niets als uitkomst te herkennen, en staat alles als gewone regel:
 * liever een bedrag tussen de vaste velden dan een vast veld als "berekend".
 *
 * Elk deel heeft de vorm van `readLexostatus`, zodat dezelfde weergave het kan
 * tonen.
 */
export function readAfwijzing(answer, cell) {
  const values = answer?.established ? answer.values ?? [] : [];
  const valueOf = (name) => values.find((value) => value.name === name)?.value;
  if (valueOf('decision_type') !== AFWIJZING) return null;
  const gronden = readAfwijzingsgronden(valueOf('afwijzingsgrond'));
  const genoemd = new Set(gronden.map((grond) => grond.output));
  const uitkomsten = besluitOutputs(cell, valueOf('besluit'));
  return {
    gronden,
    berekend: {
      ...answer,
      values: values.filter((value) => uitkomsten.has(value.name) && !genoemd.has(value.name)),
    },
    vast: {
      ...answer,
      values: values.filter(
        (value) => !uitkomsten.has(value.name) && !genoemd.has(value.name) && value.name !== 'afwijzingsgrond',
      ),
    },
  };
}

/**
 * Hoe de cel aan haar antwoord kwam: de regel in één zin, en de grammen die ze
 * las.
 *
 * Het contract is `Reductie` uit `packages/simulator/src/cell/reductie.rs`. Er
 * wordt hier niets uitgerekend: de zin wordt opgeschreven uit wat het blok zegt,
 * en elk gram blijft de verwijzing die het is — met dezelfde `id` als in het
 * journaal, waarmee het in het tabblad Grammen terug te vinden is.
 *
 * `null` als een antwoord geen uitleg draagt. Dat komt niet van een reductie
 * (die draagt er altijd een), en dan hoort er ook niets te staan in plaats van
 * een lege uitklap die iets belooft.
 */
export function readReductie(reductie) {
  const vorm = reductie?.vorm;
  if (!vorm) return null;
  return {
    soort: vorm.soort ?? null,
    zin: describeReductie(vorm),
    grams: (reductie.grammen ?? []).map((gram) => ({
      id: gram.id,
      cell: gram.cell,
      chronicle: gram.chronicle,
      kind: gram.kind,
      name: gram.name,
      volgnummer: gram.volgnummer,
      opMoment: gram.op_moment,
      regulationValidFrom: gram.regulation_valid_from ?? null,
      bijdrage: gram.bijdrage ?? null,
    })),
    inputs: (vorm.inputs ?? []).map((input) => ({
      name: input.name,
      gram: input.herkomst?.gram ?? null,
      zin: describeHerkomst(input.herkomst),
    })),
    gemist: reductie.gemist ?? null,
  };
}

/** De gelijkheden uit een `where`, als één stuk tekst. */
function describeConditions(conditions) {
  return Object.entries(conditions ?? {})
    .map(([field, value]) => `${field} = ${formatValue(value)}`)
    .join(' en ');
}

/** De regel van deze reductie in één zin, in de woorden van het beeld. */
export function describeReductie(vorm) {
  if (vorm?.soort === 'wetsvorm') {
    const versie = vorm.regulation_valid_from ? ` (versie ${formatMoment(vorm.regulation_valid_from)})` : '';
    return `uitkomst '${vorm.output}' van regeling '${vorm.regulation}'${versie}, berekend op ${formatMoment(vorm.op_moment)}`;
  }
  if (vorm?.soort === 'openstaand') {
    return `verplichtingen uit kroniek '${vorm.beschikkingen}' min de betalingen in kroniek '${vorm.betalingen}', met ${vorm.key} '${formatValue(vorm.key_value)}' op of vóór ${formatMoment(vorm.op_moment)}`;
  }
  if (vorm?.soort !== 'kroniekfilter') return '';
  const regel =
    vorm.regel?.regel === 'som' ? `som over ${vorm.regel.field}` : 'laatste vastlegging';
  const voorwaarden = describeConditions(vorm.where);
  const erbij = voorwaarden ? `, en ${voorwaarden}` : '';
  return `${regel} in kroniek '${vorm.chronicle}' met ${vorm.key} '${formatValue(vorm.key_value)}' op of vóór ${formatMoment(vorm.op_moment)}${erbij}`;
}

/** Waar één input van een wetsvorm vandaan kwam, als tekst. */
export function describeHerkomst(herkomst) {
  switch (herkomst?.herkomst) {
    case 'parameter':
      return herkomst.parameter ? `uit de vraag, parameter '${herkomst.parameter}'` : 'vast in de definitie';
    case 'eigen_kroniek':
      return `uit eigen kroniek '${herkomst.chronicle}'`;
    case 'regeling':
      return `berekend door regeling '${herkomst.regulation}' (${herkomst.output})`;
    case 'cel':
      return `van cel '${herkomst.cell}' (${herkomst.output})`;
    default:
      return '';
  }
}

/** De instellingen van de wereld, met of ze al vastliggen en waardoor. */
export function settingRows(snapshot) {
  const locked = snapshot?.locked_settings ?? {};
  return Object.entries(snapshot?.settings ?? {}).map(([name, value]) => ({
    name,
    value,
    locked: locked[name] ?? null,
  }));
}

/** De contacten over een celgrens, zoals het meetinstrument ze zag. */
export function crossings(snapshot) {
  return Array.isArray(snapshot?.crossings) ? snapshot.crossings : [];
}

/**
 * Alles wat de wereld meldde zonder het tegen te houden.
 *
 * Meer dan één soort, elk met haar eigen `soort`: een verstreken termijn is iets
 * anders dan een regeling die geen bevoegd gezag declareert. Wie ze op één hoop
 * toont, zegt over de ene wat er over de andere staat.
 */
export function warnings(snapshot) {
  return Array.isArray(snapshot?.warnings) ? snapshot.warnings : [];
}

/** De termijnen die verstreken zonder dat het feit er lag. */
export function missedDeadlines(snapshot) {
  return warnings(snapshot).filter((warning) => warning?.soort === 'gemiste_termijn');
}

/**
 * Elk gram van elke cel, op één hoop en in chronologische volgorde.
 *
 * De kolommen per cel laten zien wat één cel weet; dit laat zien wat er in de
 * hele wereld ligt, in de volgorde waarin het gebeurde. Dat is iets wat geen
 * enkele cel kan zien — net als het observatielog is dit een leesbeeld van de
 * opstelling en geen weg naar een kroniek van een ander.
 *
 * De volgorde is volledig bepaald: eerst het moment, dan de cel, dan de kroniek,
 * dan de plek in die kroniek. De dag is de korrel, dus zonder die staartsortering
 * zouden grammen van dezelfde dag per beeld van plek kunnen wisselen, en dan
 * beweegt een tabel terwijl er niets gebeurd is.
 *
 * `gram` is het gram zoals het beeld het geeft — ongefilterd, want een lezer die
 * het ruwe gram wil zien, wil precies dat zien en niet een uittreksel ervan.
 */
export function allGrams(snapshot) {
  const rows = [];
  for (const cell of cells(snapshot)) {
    for (const chronicle of chronicles(cell)) {
      (chronicle.grams ?? []).forEach((gram, index) => {
        rows.push({
          id: gramId(cell.id, chronicle.stream, index),
          cell: cell.id,
          chronicle: chronicle.stream,
          index,
          kind: gram.kind,
          name: gram.name,
          intake: gram.intake ?? '',
          grondslag: gram.grondslag ?? '',
          opMoment: gram.op_moment,
          zaak: describeZaak(gram),
          gram,
        });
      });
    }
  }
  return rows.sort(
    (a, b) =>
      String(a.opMoment).localeCompare(String(b.opMoment)) ||
      String(a.cell).localeCompare(String(b.cell)) ||
      String(a.chronicle).localeCompare(String(b.chronicle)) ||
      a.index - b.index,
  );
}
