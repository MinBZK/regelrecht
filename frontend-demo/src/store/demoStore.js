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
  evaluateLaw as engineEvaluate,
  prepareEngine,
  registerClaims,
  registerPersonaData,
} from '../engine/useDemoEngine.js';
import { DELEGATION_TYPE_LABELS, delegationKey, delegationsFor, maySubmitClaims } from '../data/delegation.js';
import { verdictOf } from '../data/format.js';
import { driftOf } from '../data/caseDrift.js';

const STORAGE_KEY = 'rr-demo-state-v1';

function today() {
  // Local calendar date, not UTC: in the evening the two differ.
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

function defaultState() {
  return {
    profileKey: null, // null = config default
    referenceDate: today(),
    // Uit, zoals in de POC (`SAMPLE_RATE = 0.0`): een aanvraag waarbij de
    // burger niets gewijzigd heeft, rekent de wet zelf af en wordt direct
    // toegekend. De presentator kan het aanzetten om te laten zien hoe een
    // behandelaar dezelfde zaak ziet.
    manualReview: false,
    cases: [],
    claims: [],
    presenterName: '',
    // Namens wie er gehandeld wordt: null is voor zichzelf. Bewaard als
    // sleutel (`BUSINESS:85234567`), niet als het hele object, want de
    // machtiging zelf komt uit de wet en wordt bij het laden opnieuw bepaald.
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
const corpus = shallowRef(null);
const engine = shallowRef(null);
const ready = ref(false);
const loadError = ref(null);
/** Bumped whenever registered data changed; views re-evaluate on it. */
const dataVersion = ref(0);
let bootPromise = null;

function newId(prefix) {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

function nowIso() {
  return new Date().toISOString();
}

/** Cases in the shape the materialiser's `kind: cases` bindings expect. */
function casesForMaterialiser() {
  return state.cases.map((c) => ({
    law: c.lawPath,
    service: c.service,
    status: c.status,
    approved: c.approved,
    bsn: c.bsn,
    kvk_nummer: c.kvk ?? null,
    year: Number(c.submittedAt?.slice(0, 4)),
    ...(c.parameters ?? {}),
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
      corpus.value = markRaw(await loadCorpus());
      pruneStaleRecords(corpus.value);
      engine.value = markRaw(await prepareEngine(corpus.value));
      reregister();
      ready.value = true;
    } catch (e) {
      loadError.value = e;
      throw e;
    }
  })();
  return bootPromise;
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
export const FEATURES = [
  // `hint` beschrijft wat de vlag aanzet en staat niet in het menu: het
  // `details`-attribuut van nldd-menu-item is een kort label rechts, en een
  // hele zin daarin perst het label op een smal scherm in een kolom van één
  // woord breed. Het blijft hier staan als uitleg bij de vlag zelf.
  { key: 'DELEGATION', label: 'Machtigingen', icon: 'switch', hint: 'Handelen namens een kind of een onderneming' },
  { key: 'CHANGE_WIZARD', label: 'Wijziging doorgeven', icon: 'edit', hint: 'Eén ingang voor inkomen, huur, adres en huishouden' },
  { key: 'HARMONIZE', label: 'Harmonisatie', icon: 'chart-x-y-axis-line', hint: 'Eén staffel, op het simulatietabblad' },
  // Geen vinkje-achtig icoon: het menu-item zet er zelf al een vinkje voor als
  // de vlag aan staat, en twee vinkjes naast elkaar leest als een fout.
  { key: 'AUTO_APPROVE_CLAIMS', label: 'Correcties direct goedkeuren', icon: 'lightning', hint: 'Zonder tussenkomst van een behandelaar' },
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
  if (!state.delegationKey) return null;
  const found = delegations.value.find((d) => delegationKey(d) === state.delegationKey) ?? null;
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

function setDelegation(delegation) {
  const key = delegationKey(delegation);
  // 'Mezelf' is geen machtiging maar de afwezigheid ervan.
  state.delegationKey = !delegation || delegation.subjectType === 'SELF' ? null : key;
}

/**
 * Parameters the portal passes to a law for the active persona.
 *
 * Handelt iemand namens een ander, dan gaan de parameters over die ander: een
 * onderneming wordt op haar KvK-nummer bevraagd, een kind op zijn BSN. Dat is
 * het hele punt van machtigen — de wet rekent over het onderwerp, niet over
 * degene die de knop indrukt.
 */
function personaParams() {
  const d = activeDelegation.value;
  if (d?.subjectType === 'BUSINESS') return { kvk_nummer: d.subjectId };
  if (d?.subjectType === 'CITIZEN') return { bsn: d.subjectId };
  const p = profile.value;
  const params = { bsn: p.bsn };
  if (p.kvk) params.kvk_nummer = p.kvk;
  return params;
}

function setProfile(key) {
  state.profileKey = key;
  // De machtigingen van het vorige profiel gelden niet voor dit profiel.
  state.delegationKey = null;
}

function setReferenceDate(date) {
  state.referenceDate = date;
  reregister();
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
 * inlogt. Het profiel bepaalt nog wel wat verborgen blijft, want dat is een
 * keuze van de demo en niet van de wet.
 */
const portalLaws = computed(() => {
  if (!corpus.value || !profile.value) return [];
  const d = activeDelegation.value;
  const wanted = d
    ? d.subjectType === 'BUSINESS' ? 'BUSINESS' : 'CITIZEN'
    : profile.value.type === 'ondernemer' ? 'BUSINESS' : 'CITIZEN';
  return [...corpus.value.latestById.values()].filter(
    (law) => law.discoverable === wanted && isLawEnabled(law),
  );
});

function evaluate(lawEntry, params = personaParams(), outputs = null) {
  return engineEvaluate(engine.value, lawEntry, params, state.referenceDate, outputs);
}

// ---- cases -----------------------------------------------------------------

/**
 * De zaak van het huidige onderwerp voor deze wet.
 *
 * Waarop vergeleken wordt volgt de wet, niet de aanroeper: een wet over een
 * onderneming (`discoverable: BUSINESS`) heeft een zaak op het KvK-nummer, een
 * wet over een persoon een zaak op de BSN. Dat onderscheid is nodig omdat een
 * ondernemer bij zichzelf allebei bij zich draagt: matchen op de BSN zolang
 * die er is, liet een ondernemer zijn eigen bedrijfszaak niet zien zodra een
 * gemachtigde die had ingediend (die zaak staat op diéns BSN), terwijl de
 * zaak wel over dezelfde onderneming ging.
 */
function findCase(lawEntry, subject = null) {
  const params = subject ?? personaParams();
  const onBusiness = lawEntry.discoverable === 'BUSINESS' && params.kvk_nummer !== undefined;
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
 * for this law or when the demo runs in "alles handmatig beoordelen" mode;
 * otherwise the decision follows the law's outcome directly.
 */
function submitCase(lawEntry, evaluation, params = personaParams()) {
  const bsn = params.bsn;
  const acting = actingOn();
  const pendingClaims = state.claims.filter(
    (c) => c.bsn === bsn && c.status === 'PENDING' && c.tileLawId === lawEntry.id,
  );
  // An unknown verdict (facts missing, RFC-036) is not a yes: the application
  // goes to a caseworker, who completes it (Awb art. 4:5) or decides.
  const verdict = verdictOf(evaluation.outputs);
  const undecided = verdict === 'unknown';
  const requirementsMet = verdict === null || verdict === true;
  const needsReview = state.manualReview || pendingClaims.length > 0 || undecided;
  const c = {
    id: newId('zaak'),
    bsn,
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
    reason: needsReview ? null : requirementsMet ? 'Automatisch toegekend op basis van de wet.' : 'Voldoet niet aan de voorwaarden.',
    submittedAt: nowIso(),
    decidedAt: needsReview ? null : nowIso(),
    objection: null,
    // Namens wie deze aanvraag is ingediend, als dat niet de persoon zelf was.
    acting,
    events: [
      {
        at: nowIso(),
        type: 'SUBMITTED',
        text: acting
          ? `Aanvraag ingediend door ${acting.actorName} namens ${acting.subjectName} (${DELEGATION_TYPE_LABELS[acting.delegationType] ?? acting.delegationType}).`
          : 'Aanvraag ingediend door de burger.',
      },
      needsReview
        ? { at: nowIso(), type: 'IN_REVIEW', text: pendingClaims.length ? 'Handmatige beoordeling: de burger heeft gegevens gewijzigd.' : undecided ? 'Handmatige beoordeling: de wet kan nog geen uitkomst geven, er ontbreken gegevens.' : 'Handmatige beoordeling (steekproef).' }
        : { at: nowIso(), type: 'DECIDED', text: requirementsMet ? 'Automatisch toegekend.' : 'Automatisch afgewezen.' },
    ],
  };
  for (const claim of pendingClaims) claim.caseId = c.id;
  state.cases.unshift(c);
  reregister();
  return c;
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
  const pendingClaims = state.claims.filter((cl) => cl.bsn === c.bsn && cl.status === 'PENDING');
  const needsReview = state.manualReview || pendingClaims.length > 0 || undecided;
  c.parameters = params;
  c.claimedResult = evaluation.outputs ?? {};
  c.verifiedResult = null;
  c.status = needsReview ? 'IN_REVIEW' : 'DECIDED';
  c.approved = needsReview ? null : requirementsMet;
  c.reason = needsReview ? null : requirementsMet ? 'Automatisch toegekend op basis van de wet.' : 'Voldoet niet aan de voorwaarden.';
  c.decidedAt = needsReview ? null : nowIso();
  c.events.push({ at: nowIso(), type: 'SUBMITTED', text: 'Aanvraag gewijzigd door de burger.' });
  c.events.push(
    needsReview
      ? {
          at: nowIso(),
          type: 'IN_REVIEW',
          text: pendingClaims.length
            ? 'Handmatige beoordeling: de burger heeft gegevens gewijzigd.'
            : undecided
              ? 'Handmatige beoordeling: de wet kan nog geen uitkomst geven, er ontbreken gegevens.'
              : 'Handmatige beoordeling (steekproef).',
        }
      : { at: nowIso(), type: 'DECIDED', text: requirementsMet ? 'Automatisch toegekend.' : 'Automatisch afgewezen.' },
  );
  // Alleen de correcties die bij déze regeling horen komen aan deze zaak te
  // hangen. De lijst hierboven is met opzet breder — een wijziging bij een
  // andere regeling telt mee voor de vraag óf er beoordeeld moet worden — maar
  // eigenaarschap is iets anders dan aanleiding. Zonder dit onderscheid raakt
  // een correctie die bij een heel andere tegel is opgegeven voorgoed aan deze
  // zaak vast (`if (!claim.caseId)` zet hem maar één keer), en staat hij in het
  // dossier van een besluit waar hij niets mee te maken heeft.
  for (const claim of pendingClaims) {
    if (!claim.caseId && claim.tileLawId === c.lawId) claim.caseId = c.id;
  }
  reregister();
  return c;
}

function decideCase(caseId, approved, reason, verifiedResult = null) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c) return;
  c.status = 'DECIDED';
  c.approved = approved;
  c.reason = reason;
  c.verifiedResult = verifiedResult;
  c.decidedAt = nowIso();
  c.events.push({ at: nowIso(), type: 'DECIDED', text: `${approved ? 'Toegekend' : 'Afgewezen'} door behandelaar: ${reason}` });
  reregister();
}

function moveCase(caseId, status) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c || c.status === status) return;
  c.status = status;
  c.events.push({ at: nowIso(), type: status, text: `Status gewijzigd naar ${status}.` });
  reregister();
}

function objectToCase(caseId, reason) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c) return;
  c.objection = { reason, status: 'PENDING', filedAt: nowIso() };
  c.status = 'IN_REVIEW';
  c.events.push({ at: nowIso(), type: 'OBJECTION', text: `Bezwaar ingediend: ${reason}` });
}

function decideObjection(caseId, upheld, reason) {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c?.objection) return;
  c.objection.status = upheld ? 'UPHELD' : 'DISMISSED';
  c.objection.decidedAt = nowIso();
  c.status = 'DECIDED';
  if (upheld) c.approved = !c.approved;
  c.events.push({ at: nowIso(), type: 'OBJECTION_DECIDED', text: `Bezwaar ${upheld ? 'gegrond' : 'ongegrond'}: ${reason}` });
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
    submittedAt: nowIso(),
    decidedAt: autoApprove ? nowIso() : null,
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
  claim.decidedAt = nowIso();
  reregister();
}

function claimFor(lawId, input, bsn = subjectBsn()) {
  return state.claims.find((c) => c.lawId === lawId && c.input === input && c.bsn === bsn && c.status !== 'REJECTED') ?? null;
}

function resetState() {
  Object.assign(state, defaultState());
  reregister();
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
    setReferenceDate,
    reregister,
    portalLaws,
    isLawEnabled,
    evaluate,
    findCase,
    caseDrift,
    submitCase,
    resubmitCase,
    decideCase,
    moveCase,
    objectToCase,
    decideObjection,
    submitClaim,
    decideClaim,
    claimFor,
    resetState,
  };
}
