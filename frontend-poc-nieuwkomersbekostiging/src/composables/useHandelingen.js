/**
 * useHandelingen - het uitvoeringslastmodel (data/handelingen.yaml) als
 * bewerkbare parameters.
 *
 * De basis komt uit /data/handelingen.yaml; een variant-branch kan een eigen
 * handelingen.yaml meebrengen (manifest-entry `data` in variants.json). De
 * sessie-overrides (minuten per handeling, tarieven, fracties, investeringen,
 * budget) liggen als reactieve laag over beide, zodat één knop in de
 * ParameterPanel alle kolommen tegelijk raakt. Aggregatie gebeurt in
 * metrics.js op de hoofdthread: bijstellen kost geen hersimulatie.
 */
import { ref, reactive, computed } from 'vue';
import yaml from 'js-yaml';
import { b } from '../basePad.js';
import { useLawStore } from '../engine/lawStore.js';

const { variants } = useLawStore();

const base = ref(null);
const variantDocs = reactive({}); // variantId -> geparste handelingen.yaml
const loaded = ref(false);
const loadError = ref(null);
let loadPromise = null;

const overrides = reactive({
  minuten: {}, // handeling-id -> minuten
  tarieven: {}, // tariefnaam -> eurocent per uur
  fracties: {}, // naam -> ratio
  investeringen: {}, // investering-id -> { bedrag?, afschrijving_jaren?, vanaf_jaar? }
  budget: {}, // post -> eurocent
});

async function fetchYaml(url) {
  // Basis hier, niet bij de aanroepers: `file.path` komt uit variants.json en
  // is relatief, net als het pad hieronder.
  const volledig = b(url);
  const res = await fetch(volledig);
  if (!res.ok) throw new Error(`Ophalen mislukt (${res.status}): ${volledig}`);
  return yaml.load(await res.text());
}

async function fetchHandelingen() {
  if (loadPromise) return loadPromise;
  loadPromise = (async () => {
    try {
      base.value = await fetchYaml('data/handelingen.yaml');
    } catch (e) {
      loadError.value = e;
      base.value = { tarieven: {}, handelingen: [], fracties: {}, investeringen: [], budget: {} };
    }
    loaded.value = true;
    return base.value;
  })();
  return loadPromise;
}

/** Haal (eenmalig) de variant-specifieke handelingen.yaml op, als de branch er een heeft. */
async function ensureVariantDoc(variantId) {
  if (!variantId || variantDocs[variantId] !== undefined) return;
  const variant = variants.value.find((v) => v.id === variantId);
  // `base` is hier een sleutel, geen URL: hij moet letterlijk gelijk zijn aan
  // wat copy-assets.js in variants.json schrijft, en dat is relatief.
  const file = variant?.data?.find((f) => f.base === 'data/handelingen.yaml');
  if (!file) {
    variantDocs[variantId] = null; // geen eigen model: basis gebruiken
    return;
  }
  try {
    variantDocs[variantId] = await fetchYaml(file.path);
  } catch {
    variantDocs[variantId] = null;
  }
}

/** Het effectieve model voor een kolom: basis of variant-doc, met overrides. */
function handelingenFor(variantId = null) {
  const doc = (variantId && variantDocs[variantId]) || base.value;
  if (!doc) return null;
  const tarieven = { ...(doc.tarieven ?? {}), ...overrides.tarieven };
  const fracties = { ...(doc.fracties ?? {}), ...overrides.fracties };
  const budget = { ...(doc.budget ?? {}), ...overrides.budget };
  const handelingen = (doc.handelingen ?? []).map((h) => ({
    ...h,
    minuten: overrides.minuten[h.id] ?? h.minuten,
  }));
  const investeringen = (doc.investeringen ?? []).map((inv) => ({
    ...inv,
    ...(overrides.investeringen[inv.id] ?? {}),
  }));
  return { ...doc, tarieven, fracties, budget, handelingen, investeringen };
}

/** Alle handelingen over basis + geladen varianten, uniek per id (voor de editor). */
const alleHandelingen = computed(() => {
  const seen = new Map();
  const docsList = [base.value, ...Object.values(variantDocs)].filter(Boolean);
  for (const doc of docsList) {
    for (const h of doc.handelingen ?? []) {
      if (!seen.has(h.id)) seen.set(h.id, h);
    }
  }
  return [...seen.values()];
});

const alleInvesteringen = computed(() => {
  const seen = new Map();
  const docsList = [base.value, ...Object.values(variantDocs)].filter(Boolean);
  for (const doc of docsList) {
    for (const inv of doc.investeringen ?? []) {
      if (!seen.has(inv.id)) seen.set(inv.id, inv);
    }
  }
  return [...seen.values()];
});

const alleTarieven = computed(() => {
  const out = {};
  const docsList = [base.value, ...Object.values(variantDocs)].filter(Boolean);
  for (const doc of docsList) Object.assign(out, doc.tarieven ?? {});
  return out;
});

const hasOverrides = computed(() =>
  Object.values(overrides).some((group) => Object.keys(group).length > 0),
);

function setMinuten(id, minuten) {
  overrides.minuten[id] = minuten;
}
function setTarief(naam, eurocentPerUur) {
  overrides.tarieven[naam] = eurocentPerUur;
}
function setFractie(naam, ratio) {
  overrides.fracties[naam] = ratio;
}
function setInvestering(id, patch) {
  overrides.investeringen[id] = { ...(overrides.investeringen[id] ?? {}), ...patch };
}
function setBudget(post, eurocent) {
  overrides.budget[post] = eurocent;
}
function resetOverrides() {
  for (const group of Object.values(overrides)) {
    for (const key of Object.keys(group)) delete group[key];
  }
}

/**
 * Het effectieve model van een kolom als yaml-tekst, voor de beleidsassistent.
 * Inclusief de sessie-overrides, want de assistent moet lezen en rekenen op
 * wat de gebruiker ziet -- net als bij de regelgeving. Zonder dit kreeg hij
 * altijd de basis, en bestond een handeling die alleen in een variant staat
 * (de po-accountant van nk-3) voor hem niet.
 */
function handelingenYamlFor(variantId = null) {
  const doc = handelingenFor(variantId);
  return doc ? yaml.dump(doc, { lineWidth: 100, noRefs: true }) : null;
}

/**
 * Neem het uitvoeringslastmodel over dat de beleidsassistent heeft bewerkt.
 * Dit vervangt de basis, niet een variant-doc: de assistent werkt op de
 * werkversie, en handelingenFor() valt voor elke kolom zonder eigen
 * handelingen.yaml op die basis terug. De sessie-overrides blijven staan,
 * zodat een knop die de gebruiker zelf verzette niet stil terugspringt.
 */
function applyHandelingenYaml(yamlText) {
  if (typeof yamlText !== 'string' || !yamlText.trim()) return false;
  try {
    const doc = yaml.load(yamlText);
    if (!doc || !Array.isArray(doc.handelingen)) return false;
    base.value = doc;
    return true;
  } catch {
    return false;
  }
}

export function useHandelingen() {
  return {
    base,
    loaded,
    loadError,
    overrides,
    hasOverrides,
    alleHandelingen,
    alleInvesteringen,
    alleTarieven,
    fetchHandelingen,
    ensureVariantDoc,
    handelingenFor,
    handelingenYamlFor,
    applyHandelingenYaml,
    setMinuten,
    setTarief,
    setFractie,
    setInvestering,
    setBudget,
    resetOverrides,
  };
}
