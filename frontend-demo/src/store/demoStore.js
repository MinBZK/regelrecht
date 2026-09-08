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

const STORAGE_KEY = 'rr-demo-state-v1';

function today() {
  return new Date().toISOString().slice(0, 10);
}

function defaultState() {
  return {
    profileKey: null, // null = config default
    referenceDate: today(),
    manualReview: true,
    cases: [],
    claims: [],
    presenterName: '',
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

function approvedClaimsForEngine() {
  return state.claims
    .filter((c) => c.status === 'APPROVED')
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
  registerPersonaData(engine.value, corpus.value, state.referenceDate, casesForMaterialiser());
  registerClaims(engine.value, approvedClaimsForEngine());
  dataVersion.value += 1;
}

async function boot() {
  if (bootPromise) return bootPromise;
  bootPromise = (async () => {
    try {
      corpus.value = markRaw(await loadCorpus());
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

/** Parameters the portal passes to a law for the active persona. */
function personaParams() {
  const p = profile.value;
  const params = { bsn: p.bsn };
  if (p.kvk) params.kvk_nummer = p.kvk;
  return params;
}

function setProfile(key) {
  state.profileKey = key;
}

function setReferenceDate(date) {
  state.referenceDate = date;
  reregister();
}

/** Is a law shown on the active profile's portal? */
function isLawEnabled(lawEntry) {
  const cfg = corpus.value?.config ?? {};
  const inList = (map) => (map?.[lawEntry.service] ?? []).some((p) => lawEntry.law_path === p || lawEntry.law_path.startsWith(`${p}/`));
  if (inList(cfg.hidden_laws)) return false;
  if (inList(profile.value?.disabled_laws)) return false;
  return true;
}

/** Laws the active persona can discover on their portal. */
const portalLaws = computed(() => {
  if (!corpus.value || !profile.value) return [];
  const wanted = profile.value.type === 'ondernemer' ? 'BUSINESS' : 'CITIZEN';
  return [...corpus.value.latestById.values()].filter(
    (law) => law.discoverable === wanted && isLawEnabled(law),
  );
});

function evaluate(lawEntry, params = personaParams(), outputs = null) {
  return engineEvaluate(engine.value, lawEntry, params, state.referenceDate, outputs);
}

// ---- cases -----------------------------------------------------------------

function findCase(lawEntry, bsn = profile.value?.bsn) {
  return state.cases.find((c) => c.lawId === lawEntry.id && c.bsn === bsn && c.status !== 'WITHDRAWN') ?? null;
}

/**
 * Submit an application. Goes to manual review when the citizen changed data
 * for this law or when the demo runs in "alles handmatig beoordelen" mode;
 * otherwise the decision follows the law's outcome directly.
 */
function submitCase(lawEntry, evaluation, params = personaParams()) {
  const bsn = params.bsn;
  const pendingClaims = state.claims.filter(
    (c) => c.bsn === bsn && c.status === 'PENDING' && c.tileLawId === lawEntry.id,
  );
  const requirementsMet = evaluation.outputs?.voldoet_aan_voorwaarden !== false;
  const needsReview = state.manualReview || pendingClaims.length > 0;
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
    events: [
      { at: nowIso(), type: 'SUBMITTED', text: 'Aanvraag ingediend door de burger.' },
      needsReview
        ? { at: nowIso(), type: 'IN_REVIEW', text: pendingClaims.length ? 'Handmatige beoordeling: de burger heeft gegevens gewijzigd.' : 'Handmatige beoordeling (steekproef).' }
        : { at: nowIso(), type: 'DECIDED', text: requirementsMet ? 'Automatisch toegekend.' : 'Automatisch afgewezen.' },
    ],
  };
  for (const claim of pendingClaims) claim.caseId = c.id;
  state.cases.unshift(c);
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

// ---- claims ----------------------------------------------------------------

/**
 * The citizen corrects a value. `lawId` is the law that owns the input (which
 * may be a dependency of the tile's law), `tileLawId` the law shown on the tile.
 */
function submitClaim({ lawId, tileLawId, input, keyField, keyValue, oldValue, newValue, reason, evidence = null, selfDeclared = false }) {
  // A value no register holds is the citizen's own declaration and applies at
  // once; a correction of a register value waits for the caseworker unless the
  // profile auto-approves.
  const autoApprove = selfDeclared || !!profile.value?.feature_flags?.AUTO_APPROVE_CLAIMS;
  const claim = {
    id: newId('claim'),
    bsn: profile.value?.bsn,
    lawId,
    tileLawId,
    input,
    keyField,
    keyValue,
    oldValue,
    newValue,
    reason,
    evidence,
    claimant: 'BURGER',
    selfDeclared,
    status: autoApprove ? 'APPROVED' : 'PENDING',
    caseId: null,
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

function claimFor(lawId, input, bsn = profile.value?.bsn) {
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
    setProfile,
    setReferenceDate,
    portalLaws,
    isLawEnabled,
    evaluate,
    findCase,
    submitCase,
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
