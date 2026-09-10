/**
 * usePopulatieAannames - de aannames achter de synthetische debiteurenpopulatie
 * (data/distributions.yaml) als zichtbare en bewerkbare parameters.
 *
 * Zelfde opzet als in de nieuwkomerscasus: het YAML-bestand is de basis, de
 * sessie-overrides liggen er als reactieve laag overheen, en een versieteller
 * vertelt usePopulation dat de steekproef opnieuw getrokken moet worden.
 *
 * De cijfers komen uit CBS StatLine, de Stand van DUO 2023 en de Stand van de
 * Uitvoering OCW 2026 (zie de kop van distributions.yaml); wat aanname is,
 * staat als zodanig in de UI.
 */
import { ref, reactive, computed } from 'vue';

const basis = ref(null); // geparste distributions.yaml, ongewijzigd
const overrides = reactive({
  totaal: {}, // 'totaal_debiteuren' -> aantal
  regimeVerdeling: {}, // regime -> aandeel
  draagkracht: {}, // regime -> draagkrachtmeting_prior
  huishouden: {}, // huishoudtype -> aandeel
  gedrag: {}, // naam -> ratio
  inkomen: {}, // 'groei_per_jaar' -> ratio
});
const versie = ref(0); // bumpt bij elke wijziging; usePopulation hangt hieraan

export const REGIME_ORDER = ['SF15_OUD', 'SF15_NIEUW', 'SF15_LLLK', 'SF35'];

export const HUISHOUD_LABELS = {
  alleenstaand: 'Alleenstaand',
  alleenstaand_met_kind: 'Alleenstaand met kind',
  paar: 'Paar',
  paar_met_kind: 'Paar met kind',
};

/** Herkomst per blok, zodat bron en aanname in de UI uit elkaar blijven. */
export const HERKOMST = {
  totaal: 'CBS 1-1-2025: 1.132.900 oud-studenten in de aflosfase, afgerond op 1 miljoen aflossende debiteuren voor de demo.',
  regime: 'Stand van DUO 2023 en Stand van de Uitvoering OCW 2026; SF15-oud telt daar ruwweg 280 tot 300 duizend debiteuren.',
  draagkracht: 'Stand van DUO: ongeveer een derde van SF15-oud vroeg ooit een draagkrachtmeting aan. De hoge waarden voor de andere regimes zijn aanname (daar gaat het automatisch).',
  huishouden: 'CBS-tabel 71486ned, landelijke huishoudensverdeling. Toegepast op de debiteurenpopulatie is een aanname.',
  gedrag: 'Stand van de Uitvoering noemt ongeveer 12.500 partner-opt-outs; het aandeel is daaruit niet publiek herleidbaar, dus aanname.',
  inkomen: 'CBS 2021/2022, inkomen van werkenden. De mediaan is hard, de overige percentielen zijn aanname rond dat punt.',
};

function setOverride(blok, key, waarde, { min = 0, max = Infinity } = {}) {
  if (!Number.isFinite(waarde)) return;
  overrides[blok] = { ...overrides[blok], [key]: Math.min(max, Math.max(min, waarde)) };
  versie.value++;
}

export function setTotaalDebiteuren(aantal) {
  setOverride('totaal', 'totaal_debiteuren', Math.round(aantal), { min: 1000 });
}

export function setRegimeAandeel(regime, aandeel) {
  setOverride('regimeVerdeling', regime, aandeel, { max: 1 });
}

export function setDraagkrachtPrior(regime, ratio) {
  setOverride('draagkracht', regime, ratio, { max: 1 });
}

export function setHuishouden(type, aandeel) {
  setOverride('huishouden', type, aandeel, { max: 1 });
}

export function setGedrag(naam, ratio) {
  setOverride('gedrag', naam, ratio, { max: 1 });
}

export function setInkomensgroei(ratio) {
  setOverride('inkomen', 'groei_per_jaar', ratio, { min: -0.1, max: 0.2 });
}

export function resetOverrides() {
  for (const blok of ['totaal', 'regimeVerdeling', 'draagkracht', 'huishouden', 'gedrag', 'inkomen']) {
    overrides[blok] = {};
  }
  versie.value++;
}

const hasOverrides = computed(
  () => ['totaal', 'regimeVerdeling', 'draagkracht', 'huishouden', 'gedrag', 'inkomen']
    .some((b) => Object.keys(overrides[b]).length > 0),
);

/**
 * De verdelingen zoals de generator ze moet zien: de YAML met de overrides
 * erin verwerkt. Per aangeraakt blok een ondiepe kopie; de rest wordt gedeeld
 * met de basis.
 */
const effectief = computed(() => {
  const d = basis.value;
  if (!d) return null;
  versie.value; // expliciete afhankelijkheid
  const out = { ...d };

  if (Object.keys(overrides.totaal).length || Object.keys(overrides.regimeVerdeling).length) {
    out.cohorten = { ...(d.cohorten ?? {}) };
    if (overrides.totaal.totaal_debiteuren !== undefined) {
      out.cohorten.totaal_debiteuren = overrides.totaal.totaal_debiteuren;
    }
    if (Object.keys(overrides.regimeVerdeling).length) {
      out.cohorten.regime_verdeling = { ...(d.cohorten?.regime_verdeling ?? {}), ...overrides.regimeVerdeling };
    }
  }

  if (Object.keys(overrides.draagkracht).length) {
    out.regimes = {};
    for (const [naam, blok] of Object.entries(d.regimes ?? {})) {
      const prior = overrides.draagkracht[naam];
      out.regimes[naam] = prior === undefined ? blok : { ...blok, draagkrachtmeting_prior: prior };
    }
  }

  if (Object.keys(overrides.huishouden).length) {
    out.huishoudtypen = { ...(d.huishoudtypen ?? {}), ...overrides.huishouden };
  }

  if (Object.keys(overrides.gedrag).length) {
    out.gedrag = { ...(d.gedrag ?? {}), ...overrides.gedrag };
  }

  if (overrides.inkomen.groei_per_jaar !== undefined) {
    out.inkomen = { ...(d.inkomen ?? {}), groei_per_jaar: overrides.inkomen.groei_per_jaar };
  }

  return out;
});

/** De regimeverdeling zoals de generator hem trekt, genormaliseerd. */
const regimeVerdeling = computed(() => {
  const d = effectief.value;
  const rij = d?.cohorten?.regime_verdeling;
  if (!rij) return [];
  const totaal = Object.values(rij).reduce((s, w) => s + Number(w), 0) || 1;
  return REGIME_ORDER.filter((r) => rij[r] !== undefined).map((regime) => ({
    regime,
    waarde: Number(rij[regime]),
    aandeel: Number(rij[regime]) / totaal,
  }));
});

const regimeSom = computed(
  () => regimeVerdeling.value.reduce((s, r) => s + r.waarde, 0),
);

/** Huishoudverdeling, genormaliseerd zoals de generator hem trekt. */
const huishoudVerdeling = computed(() => {
  const rij = effectief.value?.huishoudtypen;
  if (!rij) return [];
  const totaal = Object.values(rij).reduce((s, w) => s + Number(w), 0) || 1;
  return Object.entries(rij).map(([type, w]) => ({
    type,
    waarde: Number(w),
    aandeel: Number(w) / totaal,
  }));
});

const totaalDebiteuren = computed(() => Number(effectief.value?.cohorten?.totaal_debiteuren ?? 0));

export function usePopulatieAannames() {
  return {
    basis,
    effectief,
    overrides,
    versie,
    hasOverrides,
    regimeVerdeling,
    regimeSom,
    huishoudVerdeling,
    totaalDebiteuren,
    setTotaalDebiteuren,
    setRegimeAandeel,
    setDraagkrachtPrior,
    setHuishouden,
    setGedrag,
    setInkomensgroei,
    resetOverrides,
  };
}
