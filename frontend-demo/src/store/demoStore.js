/**
 * Demo state: the active presenter profile, the reference date, the cases the
 * citizen submitted and the corrections (claims) they made. Persisted in
 * localStorage so a page refresh during a presentation loses nothing; the menu
 * has "Reset" to start clean.
 *
 * Also the glue between corpus, engine and views: `useDemo()` returns the
 * loaded corpus, a ready engine with persona data registered, and helpers to
 * evaluate laws and to mutate cases/claims (which re-register the claim source).
 */
import { computed, markRaw, reactive, ref, shallowRef, watch } from 'vue';
import { loadCorpus } from '../data/loadCorpus.js';
import {
  createCell,
  evaluateLaw as engineEvaluate,
  executeStage as engineExecuteStage,
  prepareEngine,
  registerClaims,
  registerPersonaData,
} from '../engine/useDemoEngine.js';
import { COMPLETE, carriedInputs, decisionDates, procedureStages, reviewedByCaseworker, statusOf } from '../data/lifecycle.js';
import { declaredClaim } from '../data/declarations.js';
import { delegationKey, delegationTypeLabel, delegationsFor, maySubmitClaims, startDelegationKey } from '../data/delegation.js';
import { verdictOf } from '../data/format.js';
import { driftOf } from '../data/caseDrift.js';
import { isEntrypointFor, subjectOf } from '../data/entrypoints.js';
import { assignClaimOwnership } from '../data/claimOwnership.js';
import { applicationValues, eventsForLaw, gramsOfCase as gramsFor, momentOn } from '../data/chronolex.js';
import { materialiseRecord, tablesFromProfiles } from '../data/materialize.js';
import { addMonths, comingDates, dayOf, decisionDue, fixedDates, nextExecution, nextMoment, periodEnd } from '../data/moments.js';
import { advanceTo as advanceClock, executeDue as executeDueOn } from '../data/clock.js';
import { readingInputs, readingPeriods, readingRows } from '../data/lexostatusView.js';
import { momentView } from '../data/chronicleView.js';
import { deliver, deliveryErrors, redeliver } from '../data/channels.js';
import { consequencesOf as consequencesFrom } from '../data/consequences.js';
import { activeLocale, t } from '../i18n/index.js';

// v3: `executedThrough` houdt per uitvoering bij wat per periode (een
// berekeningsjaar) al gevraagd is; een oude staat wordt niet omgezet.
const STORAGE_KEY = 'rr-demo-state-v3';

function today() {
  // Local calendar date, not UTC: in the evening the two differ.
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

function defaultState() {
  return {
    profileKey: null, // null = config default
    // De klok van de demo: alles wat er in een zaak gebeurt, gebeurt op deze
    // dag (`nowMoment`). Hij loopt alleen vooruit (`advanceTo`).
    referenceDate: today(),
    // Uit, zoals in de POC (`SAMPLE_RATE = 0.0`): een aanvraag waarbij de
    // burger niets gewijzigd heeft, rekent de wet zelf af en wordt direct
    // toegekend. De presentator kan het aanzetten om te laten zien hoe een
    // behandelaar dezelfde zaak ziet.
    manualReview: false,
    // Aan: een besluit gaat meteen de deur uit, zoals bij een geautomatiseerde
    // toekenning in de praktijk. Uit laat de presentator de bekendmaking als
    // eigen stap tonen (Awb 3:41), met de bezwaartermijn die pas dan begint.
    autoAnnounce: true,
    cases: [],
    claims: [],
    // De grammen van de kronieken van de cellen (chronolex, RFC-022): de
    // aanvraag zoals de wet haar vraagt, en het besluit erop. De cel leeft in
    // het geheugen van de WASM-module; dit is wat ervan bewaard blijft.
    grams: [],
    // De berichten tussen de cellen die niet aankwamen (`deliver` in
    // channels.js): de afzender, zijn gram en het kanaal, met de fout en de
    // zaak (`tag.caseId`). Elke stap van de klok, en bij het laden, biedt de
    // demo ze opnieuw aan; pas wat aankwam, verdwijnt eruit.
    outbox: [],
    presenterName: '',
    // 'zaal' of 'zelfstandig'. Zaal is de standaard: daar staat een presentator
    // voor een publiek en is het scherm van de demo. De dia's met een route
    // tonen dan géén rail ernaast, want die verkleint precies het beeld waar
    // het publiek naar kijkt. Zelfstandig is voor wie zonder presentator
    // doorklikt; dan is de rail juist het verhaal dat ontbreekt.
    presentationMode: 'zaal',
    // Namens wie er gehandeld wordt: null is voor zichzelf. Bewaard als
    // sleutel (`BUSINESS:85234567`), niet als het hele object, want de
    // machtiging zelf komt uit de wet en wordt bij het laden opnieuw bepaald.
    // null = de startmachtiging van het profiel (`start_namens`), 'SELF' =
    // bewust Mezelf gekozen. Een eigen waarde voor Mezelf, want een opgeslagen
    // staat van vóór `start_namens` heeft hier null, en die hoort bij de start.
    delegationKey: null,
    // Vlaggen die de presentator tijdens de demo heeft omgezet. Alleen wat
    // hij écht aanraakte staat hier; de rest volgt het profiel uit
    // demo-config.yaml. Zo blijft "resetten" terug naar de bedoelde opzet.
    featureOverrides: {},
  };
}

function loadState() {
  try {
    const raw = window.localStorage?.getItem(STORAGE_KEY);
    if (!raw) return defaultState();
    return { ...defaultState(), ...JSON.parse(raw) };
  } catch {
    return defaultState();
  }
}

const state = reactive(loadState());
watch(
  state,
  (s) => {
    try {
      window.localStorage?.setItem(STORAGE_KEY, JSON.stringify(s));
    } catch {
      /* storage unavailable: the demo still works for this session */
    }
  },
  { deep: true },
);

// Shallow on purpose: the corpus holds 80 parsed law documents and the engine
// is a wasm-bindgen object; wrapping either in a deep proxy would be slow and
// break the engine's private pointer access.
const loadedCorpus = shallowRef(null);
const engine = shallowRef(null);
/** Per cel van het corpus haar WasmCell; leeg als de cel niet kon starten. */
const cells = shallowRef({});
/** Waarom een cel niet startte, per cel-id. */
const cellErrors = ref({});

/**
 * Het corpus zoals elk scherm het leest, met de configuratie in de taal die
 * aanstaat.
 *
 * Hier en niet in `loadCorpus`, omdat dit de reactieve kant is: `currentLocale`
 * is een ref, dus een computed eromheen laat elk scherm dat `corpus.config`
 * leest opnieuw tekenen bij een taalwissel. Het corpus zelf wordt niet opnieuw
 * geladen; alleen welke van de configuraties eruit komt verandert.
 *
 * Het Nederlands is de terugval: zonder overlay draait de demo in het
 * Nederlands, en dat is beter dan lege dia's.
 *
 * De identiteitscheck erna is geen optimalisatie achteraf: het corpus zit in
 * een `shallowRef`, dus een vers object per aanroep zou elk scherm dat het
 * leest laten hertekenen. In de bron-taal komt hetzelfde object terug.
 */
const corpus = computed(() => {
  const c = loadedCorpus.value;
  if (!c) return null;
  const config = c.configByLocale?.[activeLocale.value] ?? c.config;
  const profiles = c.profilesByLocale?.[activeLocale.value] ?? c.profiles;
  if (config === c.config && profiles === c.profiles) return c;
  return markRaw({ ...c, config, profiles });
});
const ready = ref(false);
const loadError = ref(null);
/** Bumped whenever registered data changed; views re-evaluate on it. */
const dataVersion = ref(0);
let bootPromise = null;

function newId(prefix) {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

/**
 * Het moment van nu, op de peildatum. De demo heeft één klok: de peildatum
 * (`state.referenceDate`) is de tijd van de demo, en alles wat er in een zaak
 * gebeurt (het indienen, het besluit, de bekendmaking, de grammen in de
 * kroniek) gebeurt op die dag. Daarvoor liepen er twee tijdassen naast
 * elkaar: de grammen op de peildatum, de rest op de wandklok. Het tijdstip
 * van de dag komt van de wandklok, zodat wat na elkaar gebeurt ook na elkaar
 * staat.
 *
 * Nooit vóór wat de kroniek al heeft vastgelegd: wie de demo een dag later
 * opnieuw opent, op een vroeger uur en met dezelfde peildatum, zou anders een
 * moment krijgen dat vóór de laatste gram ligt. Dan gaat de klok vanaf die
 * gram een seconde verder.
 */
function nowMoment() {
  const now = momentOn(state.referenceDate);
  let last = null;
  for (const g of state.grams) {
    if (dayOf(g.recorded_at) === state.referenceDate && (!last || Date.parse(g.recorded_at) > Date.parse(last))) last = g.recorded_at;
  }
  if (!last || Date.parse(now) > Date.parse(last)) return now;
  const later = momentOn(state.referenceDate, new Date(Date.parse(last) + 1000));
  return Date.parse(later) > Date.parse(last) ? later : last;
}

/**
 * Cases in the shape the materialiser's `kind: cases` bindings expect: what
 * was decided (the outputs, so precario reads the area the APV granted, not
 * the area applied for), what was asked, and the case's own facts on top.
 */
function casesForMaterialiser() {
  return state.cases.map((c) => ({
    ...(c.verifiedResult ?? c.claimedResult ?? {}),
    ...(c.parameters ?? {}),
    law: c.lawPath,
    service: c.service,
    status: c.status,
    approved: c.approved,
    bsn: c.bsn,
    kvk_nummer: c.kvk ?? null,
    year: Number(c.submittedAt?.slice(0, 4)),
  }));
}

/**
 * The corrections the citizen's own view computes with: everything they have
 * submitted that has not been rejected, so a pending one counts too.
 *
 * That is the POC's behaviour (`web/routers/laws.py` calls the engine with
 * `approved=False` and feeds it every PENDING and APPROVED claim), and it is
 * the point of the portal: someone who corrects their income wants to see what
 * that would mean, not the old amount with an arrow next to it. Nothing is
 * granted by it — the case still waits for a caseworker, who recomputes with
 * approved values only.
 */
function claimsForEngine() {
  return state.claims
    .filter((c) => c.status === 'APPROVED' || c.status === 'PENDING')
    .map((c) => ({
      lawId: c.lawId,
      keyField: c.keyField,
      keyValue: c.keyValue,
      input: c.input,
      newValue: c.newValue,
    }));
}

function reregister() {
  if (!engine.value || !corpus.value) return;
  registerPersonaData(engine.value, corpus.value, state.referenceDate, casesForMaterialiser(), claimsForEngine());
  registerClaims(engine.value, claimsForEngine());
  // Het registreren wist alle gegevensbronnen van de engine, ook de kroniek
  // die het beleid van een cel terugleest (`registers:` in cell.yaml).
  for (const wasmCell of Object.values(cells.value)) wasmCell.bindRegisters(engine.value);
  dataVersion.value += 1;
}

/**
 * Drop persisted cases and claims whose persona no longer exists. The persona
 * BSNs live in profiles.yaml and can change between deploys (they moved into
 * the 999-test range once); a record that survived in localStorage from before
 * such a change refers to data the materialiser cannot produce, so it can never
 * be recomputed. A record is kept when either its BSN is a persona in
 * profiles.yaml or its KvK number belongs to a configured presenter profile.
 */
function pruneStaleRecords(c) {
  const knownBsns = new Set(Object.keys(c.profiles?.profiles ?? {}));
  const knownKvks = new Set(
    Object.values(c.config?.profiles ?? {})
      .map((p) => p.kvk)
      .filter(Boolean),
  );
  const isKnown = (rec) => knownBsns.has(String(rec.bsn)) || (rec.kvk != null && knownKvks.has(String(rec.kvk)));
  const keptCases = state.cases.filter(isKnown);
  const keptClaims = state.claims.filter(isKnown);
  const droppedCases = state.cases.length - keptCases.length;
  const droppedClaims = state.claims.length - keptClaims.length;
  if (droppedCases === 0 && droppedClaims === 0) return;
  state.cases = keptCases;
  state.claims = keptClaims;
  console.info(
    `Demo: ${droppedCases} verouderde zaken en ${droppedClaims} verouderde correcties uit de opgeslagen staat verwijderd (persona bestaat niet meer in profiles.yaml).`,
  );
}

async function boot() {
  if (bootPromise) return bootPromise;
  bootPromise = (async () => {
    try {
      loadedCorpus.value = markRaw(await loadCorpus());
      pruneStaleRecords(corpus.value);
      engine.value = markRaw(await prepareEngine(corpus.value));
      reregister();
      startCells();
      flushOutbox();
      ready.value = true;
    } catch (e) {
      loadError.value = e;
      throw e;
    }
  })();
  return bootPromise;
}

/**
 * De cellen starten met de grammen van de vorige sessie. Een cel die niet
 * start (de wet geeft een gebeurtenis geen vorm) staat in `cellErrors`; de
 * rest van de demo werkt dan gewoon, zonder kroniek.
 */
function startCells() {
  const started = {};
  const errors = {};
  for (const cell of corpus.value?.cells ?? []) {
    const own = state.grams.filter((g) => cell.events.some((e) => e.chronicle === g.chronicle));
    try {
      started[cell.id] = markRaw(createCell(engine.value, cell, own, state.referenceDate));
    } catch (e) {
      errors[cell.id] = String(e?.message ?? e);
      console.warn(`Cel ${cell.id} kon niet starten:`, e);
    }
  }
  cells.value = started;
  cellErrors.value = errors;
}

function syncGrams() {
  state.grams = Object.values(cells.value).flatMap((c) => c.grams());
}

/** De gebeurtenissen van een wet in een kroniek, met de cel die ze vastlegt. */
function chronolexFor(lawEntry) {
  const found = eventsForLaw(corpus.value?.cells, lawEntry?.doc);
  if (!found || !cells.value[found.cell.id]) return null;
  return { ...found, wasmCell: cells.value[found.cell.id] };
}

/** Wat de aanvraag om deze wet bevat volgens de wet, of null. */
function applicationShape(lawEntry) {
  const c = chronolexFor(lawEntry);
  if (!c) return null;
  try {
    return c.wasmCell.shape(engine.value, c.application.name, state.referenceDate);
  } catch (e) {
    console.warn('Vorm van de aanvraag niet af te leiden:', e);
    return null;
  }
}

/**
 * Wat de persona op de aanvraag om deze wet invult: de waarden uit het
 * profiel (`application` in demo-config.yaml), met de BSN van het onderwerp
 * en de peildatum ingevuld.
 */
function applicationValuesFor(lawEntry, shape, params = personaParams()) {
  return applicationValues(shape, profile.value?.application, {
    bsn: params.bsn,
    reference_date: state.referenceDate,
    reference_year: Number(state.referenceDate.slice(0, 4)),
  });
}

/**
 * Leg de aanvraag van een zaak vast in de kroniek van de cel. Een weigering
 * van de cel (een veld dat de wet niet vraagt) staat op de zaak; de zaak zelf
 * gaat gewoon door.
 */
function recordApplication(c, lawEntry, params) {
  const chrono = chronolexFor(lawEntry);
  if (!chrono) return;
  try {
    const shape = chrono.wasmCell.shape(engine.value, chrono.application.name, state.referenceDate);
    const gram = chrono.wasmCell.recordSubmission(
      engine.value,
      chrono.application.name,
      applicationValuesFor(lawEntry, shape, params),
      momentOn(state.referenceDate),
    );
    c.applicationGramId = gram.id;
    syncGrams();
  } catch (e) {
    c.chronicleError = String(e?.message ?? e);
  }
}

/**
 * Het besluit dat de wet in fase `stage` neemt (de vorm van de gebeurtenis,
 * `stage`): de gebeurtenis en haar vorm. Een wet met één besluit heeft er
 * één, de Zorgtoeslagwet in de procedure van de Awir twee (voorschot en
 * toekenning).
 */
function decisionAt(chrono, stage) {
  if (!stage) return null;
  for (const d of chrono.decisions) {
    const shape = chrono.wasmCell.shape(engine.value, d.name, state.referenceDate);
    if (shape.stage === stage) return { event: d.name, shape };
  }
  return null;
}

/**
 * De verwijzing van een besluit naar de aanvraag waarop het wordt genomen.
 * Hoe die verwijzing heet, zegt de wet (de vorm van het besluit).
 */
function applicationReference(event, shape, c) {
  const required = Object.entries(shape.refers_to ?? {}).filter(([, r]) => r.required);
  if (required.length !== 1) throw new Error(`${event}: geen eenduidige verwijzing naar de aanvraag`);
  return { [required[0][0]]: c.applicationGramId };
}

/** De rijen van de persona's per tabel, één keer per geladen corpus. */
let dossierTables = null;
function dossierRows() {
  const c = loadedCorpus.value;
  if (dossierTables?.corpus !== c) dossierTables = { corpus: c, rowsFor: tablesFromProfiles(c.profiles) };
  return dossierTables.rowsFor;
}

/**
 * Wat het dossier geeft voor het besluit `event` op de zaak `root`: wat de
 * fase van het besluit vraagt met herkomst DOSSIER (de wet zegt het, RFC-043)
 * en de cel niet zelf uit haar kroniek leest. Waar het dossier het heeft,
 * staat in `dossier` in demo-config.yaml, in de vorm van een binding; de
 * gegevens van de zaak die de cel leest (de BSN, het berekeningsjaar) kiezen
 * de rij.
 *
 * `inputs` gaat als `extraInputs` naar de cel; `dates` zijn de datums
 * daaruit, de momenten waarop het besluit wacht.
 */
function dossierInputs(wasmCell, event, root, now) {
  const config = corpus.value?.config?.dossier ?? {};
  const stage = wasmCell.decisionStage(engine.value, event, root, now);
  const read = wasmCell.inputsFor(engine.value, event, root, now);
  const params = Object.fromEntries(Object.entries(read).map(([k, i]) => [k, i.value]));
  const inputs = {};
  const dates = {};
  for (const input of stage.inputs ?? []) {
    const { name, type, origin } = input.parameter;
    const binding = config[input.law_id]?.[name];
    if (!binding || origin?.waarde !== 'DOSSIER' || name in read || name in inputs) continue;
    const { record } = materialiseRecord(
      { id: input.law_id, parameters: Object.keys(params), inputTypes: { [name]: type } },
      { [name]: binding },
      params,
      dossierRows(),
    );
    if (!(name in record)) continue;
    inputs[name] = { value: record[name], provenance: { source: 'dossier', service: binding.service } };
    if (type === 'date') dates[name] = record[name];
  }
  return { inputs, dates };
}

/**
 * Wat de wet in fase `stage` op de aanvraag van de zaak zou besluiten, zonder
 * het vast te leggen: de cel voert die fase uit zoals bij het besluit zelf
 * (het voorschot op de schatting, de toekenning op het inkomen), met wat het
 * dossier geeft. `null` als de wet van deze zaak geen besluit in die fase
 * vastlegt.
 */
function previewAt(c, stage) {
  const chrono = chronolexFor(corpus.value?.lawById(c.lawId));
  const found = chrono && c.applicationGramId ? decisionAt(chrono, stage) : null;
  if (!found) return null;
  const now = nowMoment();
  const refersTo = applicationReference(found.event, found.shape, c);
  const { inputs } = dossierInputs(chrono.wasmCell, found.event, c.applicationGramId, now);
  const gram = chrono.wasmCell.previewDecision(engine.value, found.event, refersTo, now, inputs);
  return { chrono, ...found, refersTo, inputs, now, gram };
}

/**
 * Leg het besluit op de aanvraag vast dat in fase `stage` valt. De cel leest
 * wat het besluit vraagt terug uit haar kroniek (de lexostatussen die de
 * gebeurtenis leest), krijgt wat het dossier geeft, en voert die fase van het
 * besluitartikel uit.
 *
 * Eerst zonder vast te leggen, in dezelfde fase: dat is het oordeel van de
 * wet waartegen het besluit wordt gelegd. De kroniek zegt niets anders dan
 * het besluit. Kan de wet nog niet beslissen, wijkt de behandelaar af van wat
 * de wet in die fase berekent, of is het besluit een weigering waar de wet
 * een toekenning vestigt, dan komt er geen gram; de zaak zegt waarom.
 */
function recordDecision(c, stage) {
  if (!c.applicationGramId) return;
  c.chronicleError = null;
  c.chronicleNoteKey = null;
  try {
    const preview = previewAt(c, stage);
    if (!preview || c.decisionGrams?.[preview.event]) return;
    const { chrono, event, shape, refersTo, inputs, now } = preview;
    const verdict = verdictOf(preview.gram.fields);
    const lawGrants = verdict === null || verdict === true;
    if (verdict === 'unknown') {
      c.chronicleNoteKey = 'zaak.chronicle.undecided';
      return;
    }
    if (c.approved !== lawGrants) {
      c.chronicleNoteKey = 'zaak.chronicle.deviates';
      return;
    }
    if (!c.approved && shape.decision_type === 'TOEKENNING') {
      c.chronicleNoteKey = 'zaak.chronicle.refusal';
      return;
    }
    const gram = chrono.wasmCell.decide(engine.value, event, refersTo, now, inputs);
    c.decisionGrams = { ...(c.decisionGrams ?? {}), [event]: gram.id };
    syncGrams();
  } catch (e) {
    c.chronicleError = String(e?.message ?? e);
  }
}

/**
 * Het besluit wordt genomen op `date`: de fase waarop de zaak wacht krijgt
 * haar dagtekening, en de cel legt het besluit van die fase vast. Is er al
 * eerder besloten (het voorschot, nu de toekenning), dan is dit een nieuw
 * besluit: het moet zelf weer worden bekendgemaakt, met een eigen
 * bezwaartermijn. Daarna legt de cel vast wat er op die dag uit voortkomt,
 * zoals een termijn in de maand van de dagtekening.
 */
function takeDecision(c, date) {
  const stage = c.stageState?.current_stage ?? null;
  c.publishedAt = null;
  c.dueStage = null;
  if (c.objection && c.objection.status !== 'PENDING') c.objection = null;
  // Legt een cel dit besluit vast, dan zegt de wet welke parameter zijn
  // dagtekening is (`dated_by`); anders wat de fase vraagt en een datum is.
  const chrono = c.applicationGramId ? chronolexFor(corpus.value?.lawById(c.lawId)) : null;
  let datedBy = null;
  try {
    datedBy = (chrono && decisionAt(chrono, stage)?.shape?.dated_by) ?? null;
  } catch (e) {
    c.chronicleError = String(e?.message ?? e);
  }
  advanceLifecycle(c, decisionDates(c, date, datedBy));
  recordDecision(c, stage);
  executeDue(c);
}

/**
 * De uitvoeringen (executogrammen) die de cel van deze wet vastlegt: de
 * gebeurtenissen waarvan de wet zegt op welke dag ze worden uitgevoerd
 * (`executed_on` in hun vorm).
 */
function executionsOf(chrono) {
  const out = [];
  for (const e of chrono.cell.events) {
    if (e.name === chrono.application.name || chrono.decisions.some((d) => d.name === e.name)) continue;
    try {
      const shape = chrono.wasmCell.shape(engine.value, e.name, state.referenceDate);
      if (shape.executed_on) out.push({ event: e.name, shape });
    } catch (err) {
      console.warn(`Vorm van ${e.name} niet af te leiden:`, err);
    }
  }
  return out;
}

/**
 * Hoe een cel een bericht over een kanaal ontvangt: op `now` vastgelegd, en
 * geldend vanaf het moment waarop het vertrok (`sentAt`, het moment van de
 * gram die het draagt), zodat het antwoord van de bank op een opdracht van
 * een gepasseerde maand vóór de opdracht van de maand erna ligt. Zonder
 * `sentAt` (opnieuw bezorgd uit de outbox) komt het nu aan. Nooit later dan
 * nu: een feit ligt nooit in de toekomst.
 */
function receiveOn(now) {
  return (to, article, refersTo, inputs, sentAt) => {
    const wasmCell = cells.value[to];
    if (!wasmCell) throw new Error(`Kanaal naar cel ${to}, die niet is gestart`);
    const at = sentAt && Date.parse(sentAt) <= Date.parse(now) ? sentAt : now;
    return wasmCell.receive(engine.value, article, refersTo, inputs, now, at);
  };
}

/** Zet wat niet aankwam in de outbox, één keer per gram en kanaal. */
function keepUndelivered(undelivered) {
  for (const u of undelivered) {
    const same = (x) => x.cellId === u.cellId && x.gramId === u.gramId && x.channel === u.channel;
    state.outbox = [...(state.outbox ?? []).filter((x) => !same(x)), u];
  }
}

/**
 * Zet op elke zaak de fout van wat er voor haar nog in de outbox staat
 * (`deliveryError`), en haal hem weg als er niets meer staat. De fout volgt
 * zo de outbox: hij verdwijnt pas als het bericht aankwam, en een nieuw
 * besluit of een stap van de klok wist hem niet zolang het er nog staat.
 */
function syncDeliveryErrors() {
  const errors = deliveryErrors(state.outbox);
  for (const c of state.cases) c.deliveryError = errors[c.id] ?? null;
}

/**
 * Breng wat cel `cellId` net vastlegde (`gram`) over de kanalen naar andere
 * cellen (`channels` in demo-config.yaml), en wat daar ontstaat weer verder:
 * de betaalopdracht naar de bank, het antwoord van de bank terug. Wat niet
 * aankomt, gaat in de outbox (met de zaak `caseId`), en de fout op de zaak.
 */
function transport(cellId, gram, now, caseId) {
  const channels = corpus.value?.config?.channels ?? [];
  if (!gram || !channels.length) return;
  const { undelivered } = deliver(gram, cellId, channels, receiveOn(now), { caseId });
  keepUndelivered(undelivered);
  syncDeliveryErrors();
}

/**
 * Bied de outbox opnieuw aan, nu (bij elke stap van de klok en bij het
 * laden). Wat aankomt verdwijnt eruit, ook een antwoord dat de ontvanger al
 * had; wat weer faalt, blijft staan met de fout op zijn zaak.
 */
function flushOutbox() {
  if (!engine.value) return;
  if (state.outbox?.length) {
    const channels = corpus.value?.config?.channels ?? [];
    const gramOf = (cellId, id) => cells.value[cellId]?.grams().find((g) => g.id === id);
    state.outbox = redeliver(state.outbox, channels, gramOf, receiveOn(nowMoment())).undelivered;
    syncGrams();
  }
  syncDeliveryErrors();
}

/**
 * De cel van een wet zoals de klok haar aanspreekt: met de engine erbij.
 * Wat de cel bij een uitvoering vastlegt, gaat meteen over de kanalen naar
 * de andere cellen, voor de zaak `caseId`. Een bericht dat niet aankomt, gaat
 * in de outbox: de uitvoering zelf is dan al vastgelegd en telt als gedaan,
 * zodat de klok die dag niet opnieuw vraagt, en het bericht wordt bij de
 * volgende stap opnieuw aangeboden. Ook kanalen die rondlopen eindigen zo in
 * de outbox, met die fout op de zaak.
 */
function celOf(chrono, caseId = null) {
  const cell = chrono.wasmCell;
  return {
    dueExecutions: (event, root, after, through, now) => cell.dueExecutions(engine.value, event, root, after, through, now),
    // Wat de cel vastlegt, gaat meteen over de kanalen verder.
    execute: (event, root, day, now, period) => {
      const gram = cell.execute(engine.value, event, root, day, now, period ?? undefined);
      transport(chrono.cell.id, gram, now, caseId);
      return gram;
    },
    previewExecution: (event, root, day, now, period) => cell.previewExecution(engine.value, event, root, day, now, period ?? undefined),
  };
}

/**
 * Leg vast wat er in deze zaak tot en met de peildatum uit de wet voortkomt:
 * per uitvoering (een betaalde termijn) de dagen die de cel uit de wet geeft,
 * elk één keer gevraagd. Of er op zo'n dag iets ontstaat, zegt de wet; de cel
 * legt alleen vast als het zo is.
 */
function executeDue(c) {
  if (!c.applicationGramId || !engine.value) return;
  const chrono = chronolexFor(corpus.value?.lawById(c.lawId));
  if (!chrono) return;
  const events = executionsOf(chrono).map((x) => x.event);
  executeDueOn(c, celOf(chrono, c.id), events, { today: state.referenceDate, now: nowMoment() });
  // Ook wat de kanalen in andere cellen vastlegden, en wat er vóór een fout lukte.
  syncGrams();
}

/**
 * Het volgende besluit van de zaak: het eerste besluit van de wet dat nog
 * geen gram heeft, in de volgorde van de stromen van de cel.
 */
function nextDecisionOf(c, chrono) {
  const d = chrono.decisions.find((x) => !c.decisionGrams?.[x.name]);
  if (!d) return null;
  return { event: d.name, shape: chrono.wasmCell.shape(engine.value, d.name, state.referenceDate) };
}

/**
 * De datums die de wet het besluit `decision` (`{event, shape}`) op de zaak
 * `c` geeft, los van de dag waarop het wordt genomen (`fixedDates`): uit een
 * voorbeeld nu en een voorbeeld een maand later. Met `after` alleen wat
 * daarna ligt. Elke datum met zijn veld (`field`).
 */
function lawFixedDates(chrono, decision, c, inputs, now, after = null) {
  const refersTo = applicationReference(decision.event, decision.shape, c);
  const preview = (at) => chrono.wasmCell.previewDecision(engine.value, decision.event, refersTo, at, inputs);
  const fields = Object.fromEntries(decision.shape.fields.map((f) => [f.name, f]));
  const later = momentOn(addMonths(state.referenceDate, 1));
  return fixedDates(preview(now), preview(later), fields, after).map((m) => ({ ...m, field: fields[m.name] }));
}

/**
 * Het besluit waarop de zaak nu wacht, als zijn moment er is: de levensloop
 * staat in zijn fase, en elke datum die het dossier ervoor geeft (de aanslag
 * van Awir 19) ligt op of vóór de peildatum. `null` anders.
 */
function dueDecision(c) {
  const chrono = chronolexFor(corpus.value?.lawById(c.lawId));
  if (!chrono || !c.applicationGramId) return null;
  try {
    const found = decisionAt(chrono, c.stageState?.current_stage);
    if (!found || c.decisionGrams?.[found.event]) return null;
    const now = nowMoment();
    const { inputs, dates } = dossierInputs(chrono.wasmCell, found.event, c.applicationGramId, now);
    // Zonder datum uit het dossier: de vaste datum die de wet het besluit
    // geeft, uit twee voorbeelden in verschillende maanden (`fixedDates`).
    let lawDates = [];
    if (Object.keys(dates).length && !Object.values(dates).some((d) => typeof d === 'string')) {
      lawDates = lawFixedDates(chrono, found, c, inputs, now).map((m) => m.date);
    }
    return decisionDue(dates, state.referenceDate, lawDates) ? found : null;
  } catch (e) {
    c.chronicleError = String(e?.message ?? e);
    return null;
  }
}

/**
 * De volgende besluiten van een zaak na het eerste van hun gebeurtenis. Een
 * aanvraag om een tegemoetkoming geldt ook voor de jaren erna (Awir 15 lid
 * 5): op dezelfde aanvraag volgt elk jaar een voorschot en een toekenning.
 * Het eerste besluit van een gebeurtenis neemt de levensloop van de zaak
 * (`decisionGrams`); elk volgend zegt de cel (`dueDecision`): over welke
 * periode het gaat, en op welke dag, als het beleid van de houder die geeft
 * (`decided_on` van de stroom). Wat het dossier ervoor geeft (de aanslag
 * over dat jaar), staat erbij. Lezen schrijft niets op de zaak.
 *
 * Per gebeurtenis `{event, period, day, decidedOn, inputs, dates}`.
 */
function followingDecisionsOf(c, chrono, now) {
  const out = [];
  for (const d of chrono.decisions) {
    if (!c.decisionGrams?.[d.name]) continue;
    const due = chrono.wasmCell.dueDecision(engine.value, d.name, c.applicationGramId, now);
    const { inputs, dates } = dossierInputs(chrono.wasmCell, d.name, c.applicationGramId, now);
    out.push({ event: d.name, period: due.period ?? null, day: due.day ?? null, decidedOn: d.decided_on ?? null, inputs, dates });
  }
  return out;
}

/**
 * Neem de volgende besluiten van de zaak waarvan het moment er is: op de dag
 * die het beleid geeft, of als elke datum die het dossier ervoor geeft is
 * geweest. De demo neemt ze zoals de wet ze neemt, zonder behandelaar en
 * zonder eigen bekendmaking: alleen als de wet het besluit geeft (een
 * voorschot of toekenning, niet onbekend en geen weigering). Daarna legt de
 * cel vast wat er op die dag uit voortkomt.
 */
function decideFollowing(c) {
  const chrono = c.applicationGramId ? chronolexFor(corpus.value?.lawById(c.lawId)) : null;
  if (!chrono) return;
  const now = nowMoment();
  const today = state.referenceDate;
  let decided = false;
  try {
    for (const f of followingDecisionsOf(c, chrono, now)) {
      const due = f.day ? f.day <= today : decisionDue(f.dates, today);
      if (!due) continue;
      const shape = chrono.wasmCell.shape(engine.value, f.event, today);
      const refersTo = applicationReference(f.event, shape, c);
      const preview = chrono.wasmCell.previewDecision(engine.value, f.event, refersTo, now, f.inputs);
      const verdict = verdictOf(preview.fields);
      if (verdict === 'unknown' || verdict === false) {
        c.chronicleNoteKey = verdict === 'unknown' ? 'zaak.chronicle.undecided' : 'zaak.chronicle.refusal';
        continue;
      }
      chrono.wasmCell.decide(engine.value, f.event, refersTo, now, f.inputs);
      c.events.push({ at: now, type: 'DECIDED', approved: true, key: 'case.event.following_decision', vars: { period: f.period?.value ?? '' } });
      decided = true;
    }
  } catch (e) {
    c.chronicleError = String(e?.message ?? e);
  }
  if (decided) {
    syncGrams();
    executeDue(c);
  }
}

/**
 * Wat de wet als volgende moment van deze zaak geeft, als verwachting en niet
 * als gram: wat er nog niet is, is geen feit (RFC-044). De cel voert de wet
 * daarvoor uit zonder vast te leggen. Elk moment draagt zijn datum, wat voor
 * moment het is en de naam van wat het geeft:
 *
 * - `execution`: de eerstvolgende maand waarin een uitvoering ontstaat (de
 *   volgende voorschottermijn), met wat de cel dan zou vastleggen;
 * - `period_end`: het eind van de periode waarover besloten is;
 * - `dossier`: een datum uit het dossier waarop het volgende besluit wacht;
 * - `law`: een datum die de wet aan een besluit geeft, al genomen of nog te
 *   nemen (de uiterste toekenningsdatum, de uiterste betaaldatum). Van een
 *   besluit dat nog komt alleen de datums die niet met de besluitdag
 *   meeschuiven.
 *
 * `{ moments, error }`: lezen schrijft niets op de zaak. Een fout van de cel
 * komt terug als `error`, voor het scherm dat de momenten toont.
 */
function momentsOf(c) {
  const none = { moments: [], error: null };
  if (!c?.applicationGramId || !engine.value) return none;
  const chrono = chronolexFor(corpus.value?.lawById(c.lawId));
  if (!chrono) return none;
  const errors = [];
  const today = state.referenceDate;
  const now = nowMoment();
  const caseGrams = gramsOfCase(c);
  const moments = [];
  const cel = celOf(chrono);
  try {
    for (const { event } of executionsOf(chrono)) {
      // De dagen na vandaag die de cel uit de wet geeft, tot een jaar
      // vooruit; de eerste waarop de wet iets laat ontstaan is het moment.
      const due = cel.dueExecutions(event, c.applicationGramId, today, addMonths(today, 13), now);
      const next = nextExecution((d) => cel.previewExecution(event, c.applicationGramId, d.day, now, d.period?.value), due);
      if (next) moments.push({ date: next.date, kind: 'execution', name: event, gram: next.gram, period: next.period });
    }
  } catch (e) {
    errors.push(String(e?.message ?? e));
  }
  for (const g of caseGrams) {
    const end = periodEnd(g.period);
    if (end && end > today && !moments.some((m) => m.kind === 'period_end' && m.date === end)) {
      moments.push({ date: end, kind: 'period_end', name: g.name, period: g.period });
    }
  }
  try {
    const next = nextDecisionOf(c, chrono);
    if (next) {
      const { inputs, dates } = dossierInputs(chrono.wasmCell, next.event, c.applicationGramId, now);
      for (const [name, date] of Object.entries(dates)) {
        if (typeof date === 'string' && date > today) moments.push({ date, kind: 'dossier', name, event: next.event });
      }
      for (const { field, ...m } of lawFixedDates(chrono, next, c, inputs, now, today)) {
        moments.push({ ...m, kind: 'law', event: next.event, provision: field?.declared_by ?? null });
      }
    }
  } catch (e) {
    errors.push(String(e?.message ?? e));
  }
  try {
    for (const f of followingDecisionsOf(c, chrono, now)) {
      if (f.day && f.day > today) moments.push({ date: f.day, kind: 'decision', event: f.event, period: f.period, provision: f.decidedOn });
      for (const [name, date] of Object.entries(f.dates)) {
        if (typeof date === 'string' && date > today) moments.push({ date, kind: 'dossier', name, event: f.event, period: f.period });
      }
    }
  } catch (e) {
    errors.push(String(e?.message ?? e));
  }
  for (const g of caseGrams) {
    const fields = gramFields(g);
    for (const m of comingDates(g, fields, today)) {
      moments.push({ ...m, kind: 'law', event: g.name, provision: fields[m.name]?.declared_by ?? null });
    }
  }
  return { moments: moments.sort((a, b) => a.date.localeCompare(b.date)), error: errors.join('; ') || null };
}

/**
 * De momenten van `momentsOf` zoals een mens ze leest (`momentView`), met de
 * fout van de cel: `{ moments, error }`.
 */
function momentViewsOf(c) {
  const { moments, error } = momentsOf(c);
  const decided = (event) => gramsOfCase(c).some((g) => g.name === event);
  return { moments: moments.map((m) => momentView(m, { corpus: corpus.value, fieldsOf: gramFields, decided })), error };
}

/** De momenten van `momentsOf`, zonder de fout: voor de klok. */
function nextMoments(c) {
  return momentsOf(c).moments;
}

/**
 * Zet de klok vooruit naar `date` en laat de cel vastleggen wat er dan in de
 * zaken is ontstaan: de termijnen van elke gepasseerde maand, en een besluit
 * waarvan het moment er is. Dat besluit neemt de demo zelf als de wet erover
 * beslist en niet elke aanvraag met de hand wordt beoordeeld (zoals bij het
 * indienen); anders komt het bij de behandelaar te liggen. Terug kan niet: een
 * gram ligt nooit in de toekomst (RFC-044), dus terug is opnieuw beginnen.
 */
function advanceTo(date) {
  if (!date || date <= state.referenceDate) return;
  const open = () => state.cases.filter((c) => statusOf(c) !== 'WITHDRAWN' && c.applicationGramId);
  for (const c of open()) c.chronicleError = null;
  advanceClock(date, {
    today: () => state.referenceDate,
    setToday: (day) => {
      state.referenceDate = day;
      reregister();
      // Wat eerder niet aankwam, eerst: het antwoord op de opdracht van
      // vorige maand hoort er te zijn vóór de opdracht van deze maand.
      flushOutbox();
    },
    cases: open,
    momentsOf: nextMoments,
    step: (c) => {
      executeDue(c);
      decideFollowing(c);
      const due = dueDecision(c);
      if (!due || c.dueStage) return;
      if (state.manualReview) {
        c.dueStage = due.shape.stage;
        c.events.push({ at: nowMoment(), type: 'IN_REVIEW', key: 'case.event.decision_due' });
        return;
      }
      decideByLaw(c, due.shape.stage);
    },
  });
  reregister();
}

/**
 * Wat de wet in de fase van het volgende besluit van de zaak zou besluiten
 * (de fase waarvan het moment er is, anders de fase waarop de levensloop
 * wacht), zonder het vast te leggen: `{event, gram}`, `{error}` als de cel
 * het niet kan zeggen, of null. Lezen schrijft niets op de zaak.
 */
function decisionPreview(c) {
  try {
    const preview = previewAt(c, c?.dueStage ?? c?.stageState?.current_stage);
    return preview ? { event: preview.event, gram: preview.gram } : null;
  } catch (e) {
    return { error: String(e?.message ?? e) };
  }
}

/**
 * De peildatum zetten vanuit het menu. Vooruit loopt de klok, met alles wat
 * er onderweg ontstaat (advanceTo). Terug kan alleen zolang er nog niets is
 * vastgelegd: een feit ligt nooit in de toekomst, dus wie terug wil met
 * zaken of grammen, begint opnieuw. Geeft 'ok' of 'reset_needed'.
 */
function setClock(date) {
  if (!date || date === state.referenceDate) return 'ok';
  if (date > state.referenceDate) {
    advanceTo(date);
    return 'ok';
  }
  if (state.cases.length || state.grams.length) return 'reset_needed';
  state.referenceDate = date;
  reregister();
  if (engine.value) startCells();
  return 'ok';
}

/** Naar het eerstvolgende moment dat de wet voor deze zaak geeft. */
function advanceToNextMoment(c) {
  advanceTo(nextMoment(nextMoments(c), state.referenceDate)?.date ?? null);
}

/**
 * Het besluit van fase `stage` zoals de wet het neemt, zonder behandelaar:
 * toegekend of afgewezen naar het oordeel van de wet in die fase. Weet de wet
 * het niet, dan komt het bij de behandelaar.
 */
function decideByLaw(c, stage) {
  let verdict;
  try {
    verdict = verdictOf(previewAt(c, stage)?.gram?.fields);
  } catch (e) {
    c.chronicleError = String(e?.message ?? e);
    return;
  }
  if (verdict === 'unknown') {
    c.dueStage = stage;
    c.events.push({ at: nowMoment(), type: 'IN_REVIEW', key: 'case.review.law_needs_more_facts' });
    return;
  }
  const grants = verdict === null || verdict === true;
  c.approved = grants;
  c.reason = null;
  c.reasonKey = grants ? 'case.reason.granted_by_law' : 'case.reason.conditions_not_met';
  c.decidedAt = nowMoment();
  c.events.push({ at: c.decidedAt, type: 'DECIDED', approved: grants, key: grants ? 'case.event.granted_auto' : 'case.event.refused_auto' });
  takeDecision(c, state.referenceDate);
  c.status = statusOf(c);
  announceIfAutomatic(c);
}

/**
 * De velden van de gebeurtenis van een gram zoals de wet ze geeft op de dag
 * dat het gram telt, per naam (`type`, `fixed`, ...). Leeg als geen cel de
 * gebeurtenis vastlegt.
 */
function gramFields(gram) {
  const shape = gramShape(gram);
  return shape ? Object.fromEntries(shape.fields.map((f) => [f.name, f])) : {};
}

/**
 * De vorm van de gebeurtenis van een gram zoals de cel haar uit de wet
 * afleidt op de dag dat het gram telt (WasmCell.shape): het vestigende
 * artikel, de soort, de fase, de velden met het artikel dat erom vraagt en
 * de verwijzingen. `null` als geen cel de gebeurtenis vastlegt of de cel de
 * vorm niet kan geven.
 */
function gramShape(gram) {
  const cell = (corpus.value?.cells ?? []).find((x) => x.events.some((e) => e.name === gram?.name));
  return cell && gram?.effective_at ? eventShapeOf(cell.id, gram.name, gram.effective_at.slice(0, 10)) : null;
}

/**
 * De vorm van gebeurtenis `event` van cel `cellId` zoals de cel haar uit de
 * wet afleidt op dag `day` (standaard de peildatum): het vestigende artikel,
 * de soort, de fase en de velden met het artikel dat erom vraagt. `null` als
 * de cel de vorm niet kan geven.
 */
function eventShapeOf(cellId, event, day = state.referenceDate) {
  void dataVersion.value;
  const wasmCell = cells.value[cellId];
  if (!wasmCell) return null;
  try {
    return wasmCell.shape(engine.value, event, day);
  } catch {
    return null;
  }
}

/**
 * Wat er op zaak `c` tot nu toe is ontvangen: de lexostatus die de cel
 * daarvoor heeft (`received` in demo-config.yaml), gelezen op het moment van
 * nu, als regels: `{ rows, error }`. Leest zij per periode (een
 * berekeningsjaar), dan een regel per periode van de zaak, met `period`. Geen regels zonder zo'n lexostatus. De
 * cel telt op, niet de demo. Lezen schrijft niets op de zaak.
 */
function receivedOf(c) {
  const chrono = c?.applicationGramId ? chronolexFor(corpus.value?.lawById(c.lawId)) : null;
  const name = chrono ? corpus.value?.config?.received?.[chrono.cell.id] : null;
  if (!name) return { rows: [], error: null };
  const { lexostatuses, error } = lexostatusesOf(chrono.cell.id);
  if (error) return { rows: [], error };
  const description = lexostatuses.find((l) => l.name === name) ?? { name, kind: 'configuration' };
  const rows = [];
  const errors = [];
  for (const period of readingPeriods(description, gramsOfCase(c))) {
    const reading = readLexostatusOf(chrono.cell.id, description, c.applicationGramId, period);
    if (reading.error) errors.push(reading.error);
    rows.push(...reading.rows.map((row) => ({ ...row, period })));
  }
  return { rows, error: errors.join('; ') || null };
}

/**
 * De lexostatussen van cel `cellId` en hoe elk de kroniek reduceert
 * (`WasmCell.lexostatuses`), met de artikelen van een beleid zoals ze op de
 * peildatum gelden: `{ lexostatuses, error }`.
 */
function lexostatusesOf(cellId) {
  void dataVersion.value;
  const wasmCell = cells.value[cellId];
  if (!wasmCell) return { lexostatuses: [], error: cellErrors.value[cellId] ?? null };
  try {
    return { lexostatuses: wasmCell.lexostatuses(engine.value, state.referenceDate), error: null };
  } catch (e) {
    return { lexostatuses: [], error: String(e?.message ?? e) };
  }
}

/**
 * Lexostatus `description` (uit `lexostatusesOf`) van cel `cellId` voor de
 * zaak die met gram `root` begon, voor de periode `period` als zij per
 * periode leest, gelezen op het moment van nu (de peildatum):
 * `{ rows, grams, error }`, de waarden zoals een mens ze leest en
 * de ids van de grammen waaruit de cel ze las. De cel leest, niet de demo.
 */
function readLexostatusOf(cellId, description, root, period = null) {
  void dataVersion.value;
  const wasmCell = cells.value[cellId];
  if (!wasmCell) return { rows: [], grams: [], error: cellErrors.value[cellId] ?? null };
  try {
    const reading = wasmCell.readLexostatus(engine.value, description.name, readingInputs(description, root, period), nowMoment());
    // Een reductie in de configuratie leest velden van grammen; een beleid
    // geeft uitvoer van zijn artikelen, en die leest als dat artikel.
    const fields = description.kind === 'configuration' ? wasmCell.lexostatusFields(engine.value, description.name, state.referenceDate) : {};
    return { rows: readingRows(reading, fields, corpus.value), grams: reading.grams, error: null };
  } catch (e) {
    return { rows: [], grams: [], error: String(e?.message ?? e) };
  }
}

/**
 * Wat er buiten de overheid gebeurt door haar besluiten, voor de persona (of
 * wie er namens gehandeld wordt): per partij onder `consequences` in
 * demo-config.yaml wat haar cel over hem vastlegde, uit zijn gegevens en de
 * kroniek van die cel (data/consequences.js).
 */
function consequencesOf(bsn = subjectBsn()) {
  void dataVersion.value;
  const cellList = corpus.value?.cells ?? [];
  return consequencesFrom(corpus.value?.config?.consequences, {
    sources: corpus.value?.profiles?.profiles?.[bsn]?.sources,
    grams: state.grams,
    cells: cellList,
    serviceOf: (cellId) => serviceOfCell(cellList.find((c) => c.id === cellId)),
  });
}

/**
 * De organisatie waaronder een cel valt: die van de wet die haar eerste
 * gebeurtenis vestigt. `null` als het corpus die wet niet kent.
 */
function serviceOfCell(cell) {
  const lawId = cell?.events?.[0]?.establishes?.split('#')[0];
  return (lawId && corpus.value?.lawById?.(lawId)?.service) ?? null;
}

/** De grammen van een zaak, in de volgorde van de kroniek. */
function gramsOfCase(c) {
  return gramsFor(state.grams, c?.applicationGramId);
}

const profileKey = computed(() => state.profileKey ?? corpus.value?.config?.default_profile ?? 'merijn');
const profile = computed(() => corpus.value?.config?.profiles?.[profileKey.value] ?? null);
const persona = computed(() => {
  const p = profile.value;
  if (!p || !corpus.value) return null;
  return corpus.value.profiles.profiles?.[p.bsn] ?? null;
});

// ---- vlaggen ---------------------------------------------------------------

/**
 * De vlaggen die de demo kent, in de volgorde waarin ze in het menu staan.
 *
 * De POC zette deze in omgevingsvariabelen (`FEATURE_*`), dus alleen te
 * wijzigen door de server opnieuw te starten. Hier hoort een presentator ze
 * midden in zijn verhaal aan te kunnen zetten, dus staan ze in het demo-menu.
 */
/**
 * Waarom een zaak met de hand beoordeeld wordt.
 *
 * Een sleutel en geen zin: de gebeurtenissen van een zaak gaan naar
 * localStorage, en een opgeslagen Nederlandse zin zou na een taalwissel
 * Nederlands blijven terwijl de rest van het scherm Engels is. De zaak bewaart
 * dus wát er gebeurde; welke woorden daarbij horen is een vraag van het moment
 * van tonen.
 */
function reviewReasonKey({ pendingClaims, undecided, assessed = false }) {
  if (pendingClaims.length) return 'case.review.citizen_changed_data';
  if (undecided) return 'case.review.law_needs_more_facts';
  if (assessed) return 'case.review.assessed_by_service';
  return 'case.review.sample';
}



/**
 * De zin bij een gebeurtenis, in de taal die aan staat.
 *
 * `key` + `vars` is wat een zaak sinds deze versie bewaart. Een zaak die al in
 * localStorage stond draagt nog een kant-en-klare Nederlandse `text`, en die
 * blijft staan: hem weggooien zou de geschiedenis van een lopende demo wissen,
 * en hem vertalen kan niet, want de woorden zijn het enige wat ervan over is.
 */
export function eventText(event) {
  if (!event) return '';
  if (event.key) return t(event.key, event.vars);
  return event.text ?? '';
}

/** Waarom een zaak is toegekend of afgewezen, in de taal die aan staat. */
export function caseReason(c) {
  if (!c) return null;
  if (c.reasonKey) return t(c.reasonKey);
  return c.reason ?? null;
}

export const FEATURES = [
  // Het label staat niet hier maar in de woordenboeken, onder
  // `app.features.<key>`: de demo is tweetalig, en een Nederlands label hier
  // zou ernaast blijven staan alsof het nog iets aanstuurde.
  //
  // `hint` beschrijft wat de vlag aanzet en staat niet in het menu: het
  // `details`-attribuut van nldd-menu-item is een kort label rechts, en een
  // hele zin daarin perst het label op een smal scherm in een kolom van één
  // woord breed. Het blijft hier staan als uitleg bij de vlag zelf.
  { key: 'DELEGATION', icon: 'switch', hint: 'Handelen namens een kind of een onderneming' },
  { key: 'CHANGE_WIZARD', icon: 'edit', hint: 'Eén ingang voor inkomen, huur, adres en huishouden' },
  { key: 'HARMONIZE', icon: 'chart-x-y-axis-line', hint: 'Eén staffel, op het simulatietabblad' },
  // Geen vinkje-achtig icoon: het menu-item zet er zelf al een vinkje voor als
  // de vlag aan staat, en twee vinkjes naast elkaar leest als een fout.
  { key: 'AUTO_APPROVE_CLAIMS', icon: 'lightning', hint: 'Zonder tussenkomst van een behandelaar' },
];

/**
 * Staat een vlag aan? Wat de presentator omzette wint van het profiel; wat hij
 * niet aanraakte volgt `demo-config.yaml`. Alles leest via deze ene plek, want
 * een schakelaar die maar de helft van de features bereikt is erger dan geen
 * schakelaar.
 */
function featureEnabled(key) {
  const override = state.featureOverrides?.[key];
  if (override !== undefined) return override;
  return !!profile.value?.feature_flags?.[key];
}

/** Reactieve vorm van `featureEnabled`, voor gebruik in een template. */
const features = computed(() => Object.fromEntries(FEATURES.map((f) => [f.key, featureEnabled(f.key)])));

function toggleFeature(key) {
  state.featureOverrides = { ...state.featureOverrides, [key]: !featureEnabled(key) };
}

/** Terug naar wat het profiel zegt, voor alle vlaggen. */
function resetFeatures() {
  state.featureOverrides = {};
}

// ---- machtigingen ----------------------------------------------------------

/**
 * Namens wie de ingelogde persoon mag handelen, volgens de wet. Elke
 * provider-wet wordt met de engine geëvalueerd, dus dit verandert mee met de
 * peildatum en met gecorrigeerde gegevens; `dataVersion` triggert dat.
 */
const delegationResult = computed(() => {
  // Lees `dataVersion` zodat een correctie of een nieuwe peildatum doorwerkt.
  dataVersion.value; // eslint-disable-line no-unused-expressions
  if (!ready.value || !profile.value?.bsn) return { delegations: [], errors: [] };
  return delegationsFor(engine.value, corpus.value, profile.value.bsn, state.referenceDate);
});

/** Of dit profiel machtigingen mag gebruiken (demo-config per profiel). */
const delegationEnabled = computed(() => features.value.DELEGATION);

/** De machtigingen die dit profiel kan kiezen; leeg als de vlag uit staat. */
const delegations = computed(() => (delegationEnabled.value ? delegationResult.value.delegations : []));

/**
 * De gekozen machtiging, of null als er voor zichzelf gehandeld wordt.
 * Een bewaarde keuze die niet meer bestaat (andere peildatum, ander profiel)
 * vervalt stil naar 'voor zichzelf': dat is de veilige kant.
 */
const activeDelegation = computed(() => {
  const start = startDelegationKey(profile.value);
  // De startmachtiging volgt uit het profiel en de wet; de vlag gaat alleen
  // over wat er in de werkbalk te kiezen is. Zonder de vlag telt een eerder
  // gemaakte keuze dus niet: wie met de vlag aan "Mezelf" koos en hem daarna
  // uitzette, begint weer namens de zaak.
  const key = (delegationEnabled.value ? state.delegationKey : null) ?? start;
  if (!key || key === SELF_KEY) return null;
  const pool = key === start ? delegationResult.value.delegations : delegations.value;
  const found = pool.find((d) => delegationKey(d) === key) ?? null;
  return found && found.subjectType !== 'SELF' ? found : null;
});

/** Mag er in de huidige context gecorrigeerd en aangevraagd worden? */
const canSubmitClaims = computed(() => maySubmitClaims(activeDelegation.value));

/**
 * De BSN waar het nu over gaat. Namens een kind is dat het kind; namens een
 * onderneming blijft het de gemachtigde, want een onderneming heeft er geen.
 */
function subjectBsn() {
  const d = activeDelegation.value;
  return d?.subjectType === 'CITIZEN' ? d.subjectId : profile.value?.bsn;
}

/** De opgeslagen keuze voor Mezelf (zie `delegationKey` in defaultState). */
const SELF_KEY = 'SELF';

function setDelegation(delegation) {
  const key = delegationKey(delegation);
  // 'Mezelf' is geen machtiging maar de afwezigheid ervan, en wel een keuze:
  // die wint van de startmachtiging van het profiel.
  state.delegationKey = !delegation || delegation.subjectType === 'SELF' ? SELF_KEY : key;
}

/**
 * Parameters the portal passes to a law for the active persona.
 *
 * Handelt iemand namens een ander, dan gaan de parameters over die ander: een
 * onderneming wordt op haar KvK-nummer bevraagd, een kind op zijn BSN. Dat is
 * het hele punt van machtigen — de wet rekent over het onderwerp, niet over
 * degene die de knop indrukt. Wie voor zichzelf handelt is een burger, ook als
 * zij een onderneming heeft: die bereikt ze via de machtiging.
 */
function personaParams() {
  const d = activeDelegation.value;
  if (d?.subjectType === 'BUSINESS') return { kvk_nummer: d.subjectId };
  if (d?.subjectType === 'CITIZEN') return { bsn: d.subjectId };
  return { bsn: profile.value.bsn };
}

function setProfile(key) {
  state.profileKey = key;
  // De machtigingen van het vorige profiel gelden niet voor dit profiel; dit
  // profiel begint weer bij zijn eigen startmachtiging.
  state.delegationKey = null;
}

/**
 * Is a law shown on the active profile's portal?
 *
 * `hidden_laws` geldt altijd: dat zijn infrastructuurwetten die nergens op een
 * portaal horen. `disabled_laws` is iets anders — het snoeit het portaal van
 * één persona bij, zodat het verhaal van díe persoon overzichtelijk blijft.
 * Zodra iemand namens een ander handelt gaat dat niet meer op: de regelingen
 * van een onderneming zijn niet weggelaten omdat de gemachtigde ze in zijn
 * eigen portaal niet wil zien. Zonder deze uitzondering staat het portaal
 * namens Merijns eigen bedrijf helemaal leeg.
 */
function isLawEnabled(lawEntry) {
  const cfg = corpus.value?.config ?? {};
  const inList = (map) => (map?.[lawEntry.service] ?? []).some((p) => lawEntry.law_path === p || lawEntry.law_path.startsWith(`${p}/`));
  if (inList(cfg.hidden_laws)) return false;
  if (!activeDelegation.value && inList(profile.value?.disabled_laws)) return false;
  return true;
}

/**
 * Laws the active persona can discover on their portal.
 *
 * Namens een onderneming zijn dat de ondernemersregelingen, namens een kind de
 * burgerregelingen: waar de wet over gaat volgt het onderwerp, niet degene die
 * inlogt. Voor zichzelf ziet ook een ondernemer de burgerregelingen. Het
 * profiel bepaalt nog wel wat verborgen blijft, want dat is een keuze van de
 * demo en niet van de wet.
 */
const portalLaws = computed(() => {
  if (!corpus.value || !profile.value) return [];
  const d = activeDelegation.value;
  const wanted = d?.subjectType === 'BUSINESS' ? 'BUSINESS' : 'CITIZEN';
  return [...corpus.value.latestById.values()].filter(
    (law) => isEntrypointFor(law.doc, wanted) && isLawEnabled(law),
  );
});

function evaluate(lawEntry, params = personaParams(), outputs = null) {
  return engineEvaluate(engine.value, lawEntry, params, state.referenceDate, outputs);
}

// ---- cases -----------------------------------------------------------------

/**
 * De zaak van het huidige onderwerp voor deze wet.
 *
 * Waarop vergeleken wordt volgt de wet, niet de aanroeper: een wet die om een
 * KvK-nummer rekent heeft een zaak op het KvK-nummer, een
 * wet over een persoon een zaak op de BSN. Dat onderscheid is nodig omdat een
 * ondernemer bij zichzelf allebei bij zich draagt: matchen op de BSN zolang
 * die er is, liet een ondernemer zijn eigen bedrijfszaak niet zien zodra een
 * gemachtigde die had ingediend (die zaak staat op diéns BSN), terwijl de
 * zaak wel over dezelfde onderneming ging.
 */
function findCase(lawEntry, subject = null) {
  const params = subject ?? personaParams();
  const onBusiness = subjectOf(lawEntry.doc) === 'BUSINESS' && params.kvk_nummer !== undefined;
  return (
    state.cases.find(
      (c) =>
        c.lawId === lawEntry.id &&
        c.status !== 'WITHDRAWN' &&
        (onBusiness ? c.kvk === params.kvk_nummer : c.bsn === params.bsn),
    ) ?? null
  );
}

/**
 * Wie de handeling verricht, als het niet de persoon zelf is. Komt in het
 * dossier terecht: een besluit dat namens een ander is aangevraagd moet
 * terug te vinden zijn bij wie dat deed.
 */
function actingOn() {
  const d = activeDelegation.value;
  if (!d) return null;
  return {
    actorBsn: profile.value?.bsn ?? null,
    actorName: profile.value?.name ?? null,
    subjectId: d.subjectId,
    subjectName: d.subjectName,
    subjectType: d.subjectType,
    delegationType: d.delegationType,
    lawId: d.lawId,
    lawName: d.lawName,
  };
}

/**
 * Submit an application. Goes to manual review when the citizen changed data
 * for this law, when the service always assesses it (`review_laws`, such as
 * Rotterdam's terrasvergunning) or when the demo runs in "alles handmatig
 * beoordelen" mode; otherwise the decision follows the law's outcome directly.
 */
function submitCase(lawEntry, evaluation, params = personaParams()) {
  const bsn = params.bsn;
  // Namens een onderneming heeft de aanvraag geen BSN in haar parameters, maar
  // de correcties van de gemachtigde staan wel op diens BSN (subjectBsn). Zonder
  // die terugval vond de zaak ze niet en werd ze op ongecontroleerde gegevens
  // automatisch toegekend. De zaak zelf houdt het BSN van haar onderwerp: wie
  // daarna voor zichzelf kijkt, ziet de zaak van de BV niet als de zijne.
  const claimsBsn = bsn ?? subjectBsn();
  const acting = actingOn();
  const pendingClaims = state.claims.filter(
    (c) => c.bsn === claimsBsn && c.status === 'PENDING' && c.tileLawId === lawEntry.id,
  );
  // An unknown verdict (facts missing, RFC-036) is not a yes: the application
  // goes to a caseworker, who completes it (Awb art. 4:5) or decides.
  const verdict = verdictOf(evaluation.outputs);
  const undecided = verdict === 'unknown';
  const requirementsMet = verdict === null || verdict === true;
  const assessed = reviewedByCaseworker(lawEntry, corpus.value?.config);
  const needsReview = state.manualReview || assessed || pendingClaims.length > 0 || undecided;
  const c = {
    id: newId('zaak'),
    bsn,
    claimsBsn,
    kvk: params.kvk_nummer ?? null,
    lawId: lawEntry.id,
    lawPath: lawEntry.law_path,
    lawName: lawEntry.name,
    service: lawEntry.service,
    parameters: params,
    claimedResult: evaluation.outputs ?? {},
    verifiedResult: null,
    status: needsReview ? 'IN_REVIEW' : 'DECIDED',
    approved: needsReview ? null : requirementsMet,
    reasonKey: needsReview ? null : requirementsMet ? 'case.reason.granted_by_law' : 'case.reason.conditions_not_met',
    submittedAt: nowMoment(),
    decidedAt: needsReview ? null : nowMoment(),
    objection: null,
    // Namens wie deze aanvraag is ingediend, als dat niet de persoon zelf was.
    acting,
    events: [
      {
        at: nowMoment(),
        type: 'SUBMITTED',
        ...(acting
          ? {
              key: 'case.event.submitted_by_agent',
              vars: {
                actor: acting.actorName,
                subject: acting.subjectName,
                kind: delegationTypeLabel(acting.delegationType),
              },
            }
          : { key: 'case.event.submitted' }),
      },
      needsReview
        ? { at: nowMoment(), type: 'IN_REVIEW', key: reviewReasonKey({ pendingClaims, undecided, assessed }) }
        : { at: nowMoment(), type: 'DECIDED', key: requirementsMet ? 'case.event.granted_auto' : 'case.event.refused_auto' },
    ],
  };
  for (const claim of pendingClaims) claim.caseId = c.id;
  state.cases.unshift(c);
  recordApplication(c, lawEntry, params);
  // De levensloop begint. De aanvraag is er, dus AANVRAAG en BEHANDELING
  // hebben hun datums; daarna wacht de engine op de besluitdatum, die er pas
  // is als er besloten wordt. Bij een aanvraag die meteen wordt toegekend,
  // volgt die in dezelfde adem.
  const vandaag = isoDate(c.submittedAt);
  advanceLifecycle(c, { aanvraag_datum: vandaag, beslistermijn_start: vandaag });
  if (!needsReview) takeDecision(c, vandaag);
  c.status = statusOf(c);
  announceIfAutomatic(c);
  reregister();
  return c;
}

/**
 * Een genomen besluit meteen bekendmaken, als de presentator dat zo heeft
 * staan. Bekendmaken blijft een eigen handeling (publishCase); deze schakelaar
 * slaat alleen de knop over.
 */
function announceIfAutomatic(c) {
  if (state.autoAnnounce && statusOf(c) === 'DECIDED' && !c.publishedAt) publishCase(c.id);
}

/**
 * De levensloop van een zaak een stuk verder brengen met de gegevens die er nu
 * zijn (RFC-008).
 *
 * De engine loopt de fasen af die de Awb aan dit soort besluit geeft, vuurt bij
 * elke fase de haken die daarbij horen, en stopt zodra hij een gegeven mist.
 * Wat hij mist staat in `pendingInputs`; de demo levert dat aan op het moment
 * dat het bestaat — een besluitdatum als de behandelaar besluit, een
 * bekendmakingsdatum als het besluit wordt verstuurd.
 *
 * De uitkomsten van de Awb (de motiveringsplicht, de termijn, de einddatum)
 * stapelen zich op in `accumulated_outputs` en worden bij de zaak bewaard. De
 * engine houdt zelf niets vast; dat is hier met opzet de taak van de demo.
 *
 * Een wet die in geen procedure zit, komt in één keer klaar. Dan blijft er geen
 * `stageState` staan en valt alles terug op de gewone status.
 */
function advanceLifecycle(c, supplied = {}) {
  const lawEntry = corpus.value?.lawById(c.lawId);
  if (!engine.value || !lawEntry) return;
  // Het uitvoerveld waar deze zaak over gaat: hetzelfde veld waarop de tegel
  // en de aanvraag rekenen.
  const outputName = primaryOutputFor(lawEntry);
  if (!outputName) return;

  // Wat een fase vraagt die nog moet komen, gaat alleen mee als het nu wordt
  // aangeleverd: een tweede bekendmaking krijgt niet de datum van de eerste.
  const carried = carriedInputs(c.lifecycleInputs, c.procedureStages, c.stageState?.current_stage);
  const params = { ...(c.parameters ?? {}), ...carried, ...supplied };
  c.lifecycleInputs = { ...(c.lifecycleInputs ?? {}), ...supplied };

  const step = engineExecuteStage(engine.value, lawEntry, outputName, c.stageState ?? null, params, state.referenceDate);
  if (!step.ok) {
    // Een levensloop die niet rekent, mag de zaak niet stukmaken: de zaak
    // houdt wat ze had en de fout is te zien waar de uitkomsten staan.
    c.lifecycleError = step.error;
    return;
  }
  c.lifecycleError = null;
  if (step.complete) {
    // Alle fasen doorlopen. De laatste stand blijft staan, zodat de termijn
    // die eruit kwam zichtbaar blijft.
    c.stageState = {
      ...(c.stageState ?? {}),
      current_stage: COMPLETE,
      accumulated_outputs: { ...(c.stageState?.accumulated_outputs ?? {}), ...step.outputs },
    };
    c.pendingInputs = [];
    return;
  }
  c.stageState = step.state;
  c.pendingInputs = step.pendingInputs;
  // Welke fasen de procedure heeft en wat elke fase is (VOORSCHOT is een
  // BESLUIT), zegt de wet die haar vastlegt.
  c.procedureStages = procedureStages(
    [...(corpus.value?.latestById?.values() ?? [])].map((l) => l.doc),
    step.state?.procedure_id,
  );
}

/**
 * Het uitvoerveld dat deze wet voor de burger beantwoordt: hetzelfde veld dat
 * de tegel groot laat zien (`dashboard_outputs`), en anders het eerste dat de
 * wet declareert.
 */
function primaryOutputFor(lawEntry) {
  const configured = corpus.value?.config?.dashboard_outputs?.[`${lawEntry.service}/${lawEntry.law_path}`];
  if (configured) return configured;
  return lawEntry.outputs?.[0] ?? null;
}

/**
 * De burger dient dezelfde aanvraag opnieuw in, met de gegevens zoals ze nu
 * zijn. Dat is de POC's `Case.reset`: geen tweede zaak naast de eerste, maar
 * dezelfde zaak die opnieuw door dezelfde beoordeling gaat, met het verloop
 * dat eraan vastzit intact.
 *
 * Dit is de weg terug uit `caseDrift`. Dat een gewijzigde aanvraag daarna
 * vrijwel altijd bij een behandelaar terechtkomt, is geen keuze van de demo:
 * er ligt een wijziging die nog niet is goedgekeurd, en daar hoort iemand
 * naar te kijken.
 */
function resubmitCase(caseId, evaluation, params = personaParams()) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c || !evaluation?.ok) return null;
  const verdict = verdictOf(evaluation.outputs);
  const undecided = verdict === 'unknown';
  const requirementsMet = verdict === null || verdict === true;
  // Per BSN, niet per wet: de wijziging die deze zaak raakt kan bij een
  // andere regeling zijn opgegeven. Dat is precies het geval waar dit voor is.
  const pendingClaims = state.claims.filter((cl) => cl.bsn === (c.claimsBsn ?? c.bsn) && cl.status === 'PENDING');
  const assessed = reviewedByCaseworker({ service: c.service, law_path: c.lawPath }, corpus.value?.config);
  const needsReview = state.manualReview || assessed || pendingClaims.length > 0 || undecided;
  c.parameters = params;
  c.claimedResult = evaluation.outputs ?? {};
  c.verifiedResult = null;
  c.approved = needsReview ? null : requirementsMet;
  c.reasonKey = needsReview ? null : requirementsMet ? 'case.reason.granted_by_law' : 'case.reason.conditions_not_met';
  c.decidedAt = needsReview ? null : nowMoment();
  c.events.push({ at: nowMoment(), type: 'SUBMITTED', key: 'case.event.amended' });
  c.events.push(
    needsReview
      ? { at: nowMoment(), type: 'IN_REVIEW', key: reviewReasonKey({ pendingClaims, undecided, assessed }) }
      : { at: nowMoment(), type: 'DECIDED', key: requirementsMet ? 'case.event.granted_auto' : 'case.event.refused_auto' },
  );
  // De lijst hierboven is met opzet breder dan deze regeling — een wijziging
  // bij een andere regeling telt mee voor de vraag óf er beoordeeld moet
  // worden — maar eigenaarschap is iets anders dan aanleiding. Zie
  // `assignClaimOwnership`.
  assignClaimOwnership(pendingClaims, c);
  // De levensloop begint opnieuw: dit is dezelfde zaak die nog eens door
  // dezelfde procedure gaat, dus ook het besluit en de bekendmaking worden
  // opnieuw gedaan. De bezwaartermijn die bij het vorige besluit hoorde geldt
  // niet meer voor dit besluit, en hoort dus niet te blijven staan.
  c.stageState = null;
  c.lifecycleInputs = {};
  c.publishedAt = null;
  // In de kroniek is dit een nieuwe aanvraag, met straks een eigen besluit
  // erop; de grammen van de vorige blijven in de kroniek staan.
  c.applicationGramId = null;
  c.decisionGrams = {};
  c.executedThrough = {};
  c.dueStage = null;
  c.chronicleNoteKey = null;
  c.chronicleError = null;
  const lawEntry = corpus.value?.lawById(c.lawId);
  if (lawEntry) recordApplication(c, lawEntry, params);
  const opnieuw = isoDate(nowMoment());
  advanceLifecycle(c, { aanvraag_datum: opnieuw, beslistermijn_start: opnieuw });
  if (!needsReview) takeDecision(c, opnieuw);
  c.status = statusOf(c);
  announceIfAutomatic(c);
  reregister();
  return c;
}

function decideCase(caseId, approved, reason, verifiedResult = null) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c) return;
  c.approved = approved;
  c.reason = reason;
  c.verifiedResult = verifiedResult;
  c.decidedAt = nowMoment();
  c.events.push({
    at: nowMoment(),
    type: 'DECIDED',
    approved,
    key: approved ? 'case.event.granted_by_officer' : 'case.event.refused_by_officer',
    vars: { reason },
  });
  // Het besluit is genomen: dat is de datum waar de fase van het besluit op
  // wachtte.
  takeDecision(c, isoDate(c.decidedAt));
  c.status = statusOf(c);
  announceIfAutomatic(c);
  reregister();
}

/**
 * Het besluit wordt bekendgemaakt (Awb 3:41): het gaat de deur uit naar de
 * belanghebbende.
 *
 * Een eigen handeling en geen bijzaak van het besluit, omdat de Awb er twee
 * verschillende momenten van maakt. RFC-008 zet de overgang BESLUIT →
 * BEKENDMAKING dan ook op "manual", en pas hier vuurt artikel 6:8 en komt de
 * bezwaartermijn als datum tevoorschijn. Vóór dit moment is er niets om
 * bezwaar tegen te maken: de burger weet van niets.
 */
function publishCase(caseId, bekendmakingDatum = null) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c || c.publishedAt) return;
  // Bekendmaken is het meedelen van een besluit (Awb 3:41), dus zonder besluit
  // is er niets mee te delen. De knop staat er ook niet eerder, maar dat is een
  // regel van de wet en hoort niet van het scherm af te hangen.
  if (statusOf(c) !== 'DECIDED') return;
  c.publishedAt = nowMoment();
  const datum = bekendmakingDatum ?? isoDate(c.publishedAt);
  c.events.push({ at: nowMoment(), type: 'BEKENDMAKING', key: 'case.event.announced', vars: { date: datum } });
  advanceLifecycle(c, { bekendmaking_datum: datum });
  c.status = statusOf(c);
  reregister();
}

/** De kalenderdatum uit een tijdstempel; de wet rekent in dagen, niet in uren. */
function isoDate(iso) {
  return String(iso ?? state.referenceDate).slice(0, 10);
}

function moveCase(caseId, status) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c || c.status === status) return;
  c.status = status;
  c.events.push({ at: nowMoment(), type: status, key: 'case.event.status_changed', vars: { status } });
  reregister();
}

/**
 * De burger maakt bezwaar (Awb 6:4).
 *
 * Het bezwaar is een veld op het besluit en niet een stap terug in de
 * levensloop: het besluit blíjft genomen en bekendgemaakt, er is alleen iets
 * tegen ingebracht. Dat de behandeling van een bezwaar strikt genomen een eigen
 * besluit is met een eigen levensloop (Awb 7:12, RFC-008 vraag 2), gaat verder
 * dan wat de demo laat zien; hier is het één veld.
 */
function objectToCase(caseId, reason) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c) return;
  c.objection = { reason, status: 'PENDING', filedAt: nowMoment() };
  c.events.push({ at: nowMoment(), type: 'OBJECTION', key: 'case.event.objection', vars: { reason } });
  reregister();
}

function decideObjection(caseId, upheld, reason) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c?.objection) return;
  c.objection.status = upheld ? 'UPHELD' : 'DISMISSED';
  c.objection.decidedAt = nowMoment();
  c.status = 'DECIDED';
  if (upheld) c.approved = !c.approved;
  c.events.push({
    at: nowMoment(),
    type: 'OBJECTION_DECIDED',
    upheld,
    key: upheld ? 'case.event.objection_upheld' : 'case.event.objection_dismissed',
    vars: { reason },
  });
  reregister();
}

/**
 * Wat een lopende aanvraag nog waard is, gegeven wat er ná het indienen is
 * gewijzigd. De vergelijking zelf staat in `data/caseDrift.js`; hier wordt
 * alleen de zaak erbij gezocht.
 */
function caseDrift(lawEntry, evaluation, subject = null) {
  // De behandelaar heeft de zaak al in handen en geeft hem mee; de portal
  // zoekt hem op bij het huidige onderwerp.
  return driftOf(subject?.id ? subject : findCase(lawEntry, subject), evaluation);
}

/**
 * De waarde die nu geldt voor één gegeven van één wet, zónder de correcties
 * die er al liggen.
 *
 * Nodig om bij een nieuwe correctie vast te leggen wat er stond. Dat kan niet
 * uit de gewone doorrekening komen: die telt openstaande correcties mee
 * (`claimsForEngine`), dus daar staat de nieuwe waarde al in en zou "oud"
 * hetzelfde zijn als "nieuw". De correctiebron gaat er daarom even af en
 * daarna weer op.
 *
 * `keyField`/`keyValue` zeggen over wie het gaat: een BSN, een KvK-nummer, een
 * organisatie. Die komen van de correctie zelf, want het onderwerp van een
 * correctie is niet altijd degene die haar indient.
 *
 * `null` als het niet lukt — een wet die niet doorrekent mag een correctie niet
 * tegenhouden; er is dan alleen niets om doorgestreept te tonen.
 */
function currentValueOf(lawId, input, keyField, keyValue) {
  const lawEntry = corpus.value?.lawById(lawId);
  if (!engine.value || !lawEntry || !keyField || keyValue == null) return null;
  try {
    registerClaims(engine.value, []);
    // Doorrekenen voor het onderwerp waar de correctie over gáát, en niet voor
    // wie haar indient. Een correctie op een gegeven van een onderneming staat
    // op het KvK-nummer; die voor de BSN van de gemachtigde doorrekenen levert
    // de waarde van een ander op, of een parameter die niet past.
    const result = evaluate(lawEntry, { [keyField]: keyValue }, [input]);
    return result.ok ? result.outputs?.[input] ?? null : null;
  } catch {
    return null;
  } finally {
    // Altijd terugzetten: zonder dit rekent de rest van de demo verder zonder
    // de correcties van de burger.
    registerClaims(engine.value, claimsForEngine());
  }
}

// ---- claims ----------------------------------------------------------------

/**
 * Someone corrects a value. `lawId` is the law that owns the input (which may
 * be a dependency of the tile's law), `tileLawId` the law shown on the tile.
 *
 * By default this is the citizen (the active profile) correcting a register
 * value from the portal. The caseworker corrects the same values from the case
 * inspector: then `claimant` is 'BEHANDELAAR', `bsn` and `caseId` are the
 * case's, and `approve` is true, because the caseworker is the one who would
 * otherwise approve it.
 *
 * Handelt iemand namens een kind, dan hoort de correctie bij dat kind: het
 * gaat om diens gegevens. Namens een onderneming blijft de correctie op het
 * KvK-nummer staan (`keyField`/`keyValue` dragen dat al), en is de BSN van de
 * gemachtigde alleen wie het deed.
 *
 * `evidence` is `{ name, type, size, dataUrl? }` for an uploaded document;
 * `hardship` is `{ clause }` when the correction appeals to a hardship clause.
 */
function submitClaim({
  lawId,
  tileLawId,
  input,
  keyField,
  keyValue,
  oldValue,
  newValue,
  reason,
  evidence = null,
  hardship = null,
  selfDeclared = false,
  claimant = 'BURGER',
  bsn = subjectBsn(),
  caseId = null,
  approve = null,
}) {
  const acting = claimant === 'BEHANDELAAR' ? null : actingOn();
  // Wat er stond vóór deze correctie. De aanroeper weet dat niet altijd — de
  // wijzigingswizard kent alleen wat de burger invult — en zonder die waarde
  // valt er later niets te tonen dan "0 → 0". Ze wordt hier uitgerekend, bij de
  // engine, en niet in elk scherm apart. Al bestaande correcties tellen niet
  // mee: de oude waarde is wat er stond, niet wat een eerdere correctie er al
  // van gemaakt had.
  const oldValueResolved = oldValue ?? currentValueOf(lawId, input, keyField, keyValue);
  // A value no register holds is the citizen's own declaration and applies at
  // once; a correction of a register value waits for the caseworker unless the
  // profile auto-approves. An appeal to a hardship clause always needs a human,
  // whatever the profile says. An explicit `approve` (the caseworker) wins.
  const autoApprove = approve ?? (hardship ? false : selfDeclared || featureEnabled('AUTO_APPROVE_CLAIMS'));
  const claim = {
    id: newId('claim'),
    bsn,
    lawId,
    tileLawId,
    input,
    keyField,
    keyValue,
    oldValue: oldValueResolved,
    newValue,
    reason,
    evidence,
    hardship,
    claimant,
    selfDeclared,
    status: autoApprove ? 'APPROVED' : 'PENDING',
    caseId,
    // Namens wie deze correctie is ingediend, als dat niet de persoon zelf was.
    acting,
    submittedAt: nowMoment(),
    decidedAt: autoApprove ? nowMoment() : null,
  };
  state.claims.unshift(claim);
  reregister();
  return claim;
}

function decideClaim(claimId, approved, reason = '') {
  const claim = state.claims.find((c) => c.id === claimId);
  if (!claim) return;
  claim.status = approved ? 'APPROVED' : 'REJECTED';
  claim.decisionReason = reason;
  claim.decidedAt = nowMoment();
  reregister();
}

function claimFor(lawId, input, bsn = subjectBsn()) {
  return (
    state.claims.find((c) => c.lawId === lawId && c.input === input && c.bsn === bsn && c.status !== 'REJECTED') ??
    // Wat het profiel al eerder opgaf, zoals Café Noon in zijn accijnsaangifte.
    declaredClaim(profile.value, lawId, input)
  );
}

function resetState() {
  Object.assign(state, defaultState());
  reregister();
  if (engine.value) startCells();
}

export function useDemo() {
  return {
    state,
    corpus,
    engine,
    ready,
    loadError,
    dataVersion,
    boot,
    profileKey,
    profile,
    persona,
    personaParams,
    features,
    toggleFeature,
    resetFeatures,
    delegations,
    delegationEnabled,
    delegationErrors: computed(() => delegationResult.value.errors),
    activeDelegation,
    canSubmitClaims,
    setDelegation,
    subjectBsn,
    setProfile,
    advanceTo,
    setClock,
    advanceToNextMoment,
    nextMoments,
    momentViewsOf,
    decisionPreview,
    receivedOf,
    lexostatusesOf,
    readLexostatusOf,
    consequencesOf,
    reregister,
    portalLaws,
    isLawEnabled,
    evaluate,
    findCase,
    caseDrift,
    submitCase,
    resubmitCase,
    decideCase,
    publishCase,
    moveCase,
    objectToCase,
    decideObjection,
    submitClaim,
    decideClaim,
    claimFor,
    resetState,
    cellErrors,
    chronolexFor,
    applicationShape,
    applicationValuesFor,
    gramFields,
    gramShape,
    eventShapeOf,
    gramsOfCase,
  };
}
