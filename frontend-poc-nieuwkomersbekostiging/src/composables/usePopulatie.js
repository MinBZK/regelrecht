/**
 * usePopulatie - de aannames achter de synthetische populatie
 * (data/distributions.yaml) als zichtbare en bewerkbare parameters.
 *
 * Dezelfde opzet als useHandelingen: het YAML-bestand is de basis, de
 * sessie-overrides liggen er als reactieve laag overheen. Anders dan bij
 * het uitvoeringslastmodel raakt een wijziging hier wél de steekproef
 * zelf, dus na een aanpassing moeten alle kolommen opnieuw draaien;
 * useSimulation kijkt daarvoor naar `versie`.
 *
 * De cijfers komen uit CBS StatLine, DUO-open data en de OCW-factsheet
 * (zie de kop van distributions.yaml); wat aanname is, staat als zodanig
 * in `herkomst` en wordt in de UI ook zo genoemd.
 */
import { ref, reactive, computed } from 'vue';

const basis = ref(null); // geparste distributions.yaml, ongewijzigd
const overrides = reactive({
  instroom: {}, // `${jaar}:${sector}` -> aantal
  categorie: {}, // `${sector}:${naam}` -> fractie
  scholen: {}, // `${sector}:aantal` -> aantal scholen
  fracties: {}, // naam -> ratio (losse fracties op het hoofdniveau)
});
const versie = ref(0); // bumpt bij elke wijziging; useSimulation hangt hieraan

/** Losse fracties op het hoofdniveau die als knop zinvol zijn. */
export const FRACTIE_KNOPPEN = [
  {
    key: 'oordeel_bevoegd_gezag_asielzoeker_fractie',
    label: 'Bestuur merkt aan als asielzoeker',
    uitleg: 'Van de leerlingen met een ambigue code of zonder BSN (tabblad 2 en 3): het deel waarvoor het bestuur ASIELZOEKER vastlegt. Aanname.',
  },
  {
    key: 'oordeel_ontbreekt_fractie',
    label: 'Bestuur legt geen oordeel vast',
    uitleg: 'Het deel daarvan waarvoor het oordeel ontbreekt; de engine geeft dan BESTUUR_BEOORDEELT. Aanname.',
  },
  {
    key: 'in_telling_1_februari_prior',
    label: 'Al in de 1-februaritelling',
    uitleg: 'Asielzoekers in het po die al in de reguliere telling van het vorige schooljaar zaten (At, art. 34 lid 9) en op 1 januari dus het lage tarief krijgen. Aanname.',
  },
  {
    key: 'werkelijk_schoolgaand_fractie',
    label: 'Werkelijk schoolgaand',
    uitleg: 'Op een peildatum daadwerkelijk op school; de rest is uitgeschreven, verhuisd of langdurig afwezig. Aanname.',
  },
];

/** Waar een blok vandaan komt; getoond bij de knoppen zodat aanname en bron uit elkaar blijven. */
export const HERKOMST = {
  instroom: 'CBS 85371NED (aankomsten naar leeftijd) en 85848NED, gekalibreerd op de OCW-factsheet; 2026 en later vlak doorgetrokken.',
  categorie: 'CBS 84809NED/84808NED (migratiemotief) en 83102NED (asielverzoeken); de verdeling binnen de eenduidige groep is een aanname.',
  scholen: 'DUO-open data voor het totaal aantal scholen; het aandeel met nieuwkomers en de staart per school zijn aanname.',
};

function setOverride(blok, key, waarde) {
  if (!Number.isFinite(waarde)) return;
  overrides[blok] = { ...overrides[blok], [key]: waarde };
  versie.value++;
}

export function setInstroom(jaar, sector, aantal) {
  setOverride('instroom', `${jaar}:${sector}`, Math.max(0, Math.round(aantal)));
}

export function setCategorie(sector, naam, fractie) {
  setOverride('categorie', `${sector}:${naam}`, Math.min(1, Math.max(0, fractie)));
}

export function setScholen(sector, aantal) {
  setOverride('scholen', `${sector}:aantal`, Math.max(1, Math.round(aantal)));
}

export function setFractie(naam, ratio) {
  setOverride('fracties', naam, Math.min(1, Math.max(0, ratio)));
}

export function resetOverrides() {
  for (const blok of ['instroom', 'categorie', 'scholen', 'fracties']) overrides[blok] = {};
  versie.value++;
}

const hasOverrides = computed(
  () => ['instroom', 'categorie', 'scholen', 'fracties'].some((b) => Object.keys(overrides[b]).length > 0),
);

/**
 * De verdelingen zoals de generator ze moet zien: de YAML met de overrides
 * erin verwerkt. Een ondiepe kopie per aangeraakt blok is genoeg; de rest
 * wordt gedeeld met de basis.
 */
const effectief = computed(() => {
  const d = basis.value;
  if (!d) return null;
  versie.value; // expliciete afhankelijkheid
  const out = { ...d };

  if (Object.keys(overrides.instroom).length) {
    const instroom = {};
    for (const [jaar, row] of Object.entries(d.instroom_per_jaar ?? {})) {
      instroom[jaar] = { ...row };
      for (const sector of ['po', 'vo']) {
        const v = overrides.instroom[`${jaar}:${sector}`];
        if (v !== undefined) instroom[jaar][sector] = v;
      }
    }
    out.instroom_per_jaar = instroom;
  }

  if (Object.keys(overrides.categorie).length) {
    const verdeling = {};
    for (const [sector, row] of Object.entries(d.categorie_verdeling ?? {})) {
      verdeling[sector] = { ...row };
      for (const naam of Object.keys(row)) {
        const v = overrides.categorie[`${sector}:${naam}`];
        if (v !== undefined) verdeling[sector][naam] = v;
      }
    }
    out.categorie_verdeling = verdeling;
  }

  if (Object.keys(overrides.scholen).length) {
    const scholen = {};
    for (const [sector, row] of Object.entries(d.scholen ?? {})) {
      scholen[sector] = { ...row };
      const v = overrides.scholen[`${sector}:aantal`];
      if (v !== undefined) scholen[sector].aantal = v;
    }
    out.scholen = scholen;
  }

  for (const [naam, waarde] of Object.entries(overrides.fracties)) out[naam] = waarde;

  return out;
});

/** De categorieverdeling per sector, genormaliseerd zoals de generator hem trekt. */
const categorieVerdeling = computed(() => {
  const d = effectief.value;
  if (!d?.categorie_verdeling) return [];
  return Object.entries(d.categorie_verdeling).map(([sector, row]) => {
    const totaal = Object.values(row).reduce((s, w) => s + Number(w), 0) || 1;
    return {
      sector,
      totaal,
      items: Object.entries(row).map(([naam, w]) => ({ naam, waarde: Number(w), aandeel: Number(w) / totaal })),
    };
  });
});

/** Instroom per jaar, met het totaal dat de weging van de records bepaalt. */
const instroomRijen = computed(() => {
  const d = effectief.value;
  if (!d?.instroom_per_jaar) return [];
  return Object.entries(d.instroom_per_jaar)
    .map(([jaar, row]) => ({
      jaar: Number(jaar),
      po: Number(row.po ?? 0),
      vo: Number(row.vo ?? 0),
      totaal: Number(row.po ?? 0) + Number(row.vo ?? 0),
    }))
    .sort((a, b) => a.jaar - b.jaar);
});

const instroomTotaal = computed(() => instroomRijen.value.reduce((s, r) => s + r.totaal, 0));

export function usePopulatie() {
  return {
    basis,
    effectief,
    overrides,
    versie,
    hasOverrides,
    categorieVerdeling,
    instroomRijen,
    instroomTotaal,
    setInstroom,
    setCategorie,
    setScholen,
    setFractie,
    resetOverrides,
  };
}
