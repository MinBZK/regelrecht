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

/** Hoe dit gram eruitziet; een onbekend soort blijft leesbaar. */
export function gramKind(kind) {
  return GRAM_KINDS[kind] ?? UNKNOWN_KIND;
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

/** Een leeg formulier voor een actie: elk veld uit `form`, met zijn type. */
export function emptyForm(action) {
  const values = {};
  for (const field of Array.isArray(action?.form) ? action.form : []) {
    values[field.name] = field.type === 'number' ? null : field.type === 'boolean' ? false : '';
  }
  return values;
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
  return '';
}

/**
 * De velden van een gram, met hun herkomst, op naam gesorteerd.
 *
 * De vaste velden van een besluit (zaakkenmerk, regeling, bevoegd gezag) staan
 * vooraan: dat is waaraan een lezer het gram herkent. Daarna komt de rest.
 */
export function gramFields(gram) {
  const entries = Object.entries(gram?.fields ?? {});
  const rank = (field) => (field.origin?.herkomst === 'besluit' ? 0 : 1);
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

/** De herkomst van een besluit-input, zoals het gram haar opschreef. */
function describeRecordedOrigin(recorded) {
  switch (recorded?.herkomst) {
    case 'geaccepteerd':
      return {
        kind: 'geaccepteerd',
        label: `geaccepteerd van cel '${recorded.cell}'`,
        color: 'oranje',
        details: pairs([
          ['lexostatus', recorded.lexostatus],
          ['uitkomst', recorded.field],
          ['op moment', recorded.op_moment],
          ['gevraagd door', recorded.asked_by],
          ['ondertekend', recorded.signature],
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
 * De verplichtingen die uit een besluit volgen, als rijen.
 *
 * Elke verplichting draagt haar eigen namen (bedrag, vervaldatum, betaler); de
 * kolommen komen daarom uit de verplichtingen zelf en niet uit een lijst hier.
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
 * Het antwoord op een lexostatus-vraag, uitgesplitst.
 *
 * "Niets vastgesteld" is een antwoord en geen fout: het komt hier als
 * `established: false` met de reden die de cel gaf.
 */
export function readLexostatus(answer) {
  const outcome = answer?.outcome ?? {};
  if (outcome.established) {
    return {
      established: true,
      cell: answer.cell ?? null,
      name: answer.name ?? null,
      opMoment: answer.op_moment ?? null,
      values: Object.entries(outcome.established).map(([key, value]) => ({ name: key, value })),
      reason: null,
    };
  }
  return {
    established: false,
    cell: answer?.cell ?? null,
    name: answer?.name ?? null,
    opMoment: answer?.op_moment ?? null,
    values: [],
    reason: outcome.not_established?.reason ?? 'In deze cel is hierover niets vastgesteld.',
  };
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

/** De termijnen die verstreken zonder dat het feit er lag. */
export function warnings(snapshot) {
  return Array.isArray(snapshot?.warnings) ? snapshot.warnings : [];
}
