/**
 * Harmonisatie: uit de uitkomsten van een simulatie één vereenvoudigde regeling
 * afleiden die de bestaande regelingen zo goed mogelijk benadert.
 *
 * De vraag erachter is een beleidsvraag. Vier toeslagen met elk hun eigen
 * voorwaarden, drempels en afbouwpaden leveren samen een bedrag op dat niemand
 * meer kan uitleggen. Wat als je diezelfde uitkomst probeert te vangen in één
 * staffel? Hoe dicht kom je, en waar zit het verschil? Dat verschil is het
 * interessante: waar het model de wet niet kan volgen, zit vaak precies de
 * regel die niemand meer begrijpt.
 *
 * De POC deed dit met scikit-learn in een Python-subprocess (`synthesize/`,
 * aangestuurd door `web/routers/harmonize.py`). Deze demo heeft geen backend —
 * de engine draait als WASM in de browser — dus is het staffelmodel hier
 * opnieuw geschreven in JavaScript. Dat kan omdat het rekenwerk elementair is:
 * kwantielen, groepsgemiddelden en kleinste kwadraten. De beslisboom en het
 * parametrische model uit de POC leunen wél op sklearn en scipy
 * (DecisionTreeRegressor, differential_evolution) en zijn hier niet
 * overgenomen.
 *
 * De invoer is een gewone simulatierun (`simulation/runner.js`): dezelfde
 * gegenereerde bevolking, dezelfde echte wetten, dezelfde engine. Er wordt
 * hier dus niets nagebootst — het model leert van wat de wet werkelijk deed.
 */

// ---- elementaire statistiek ------------------------------------------------

export function mean(values) {
  const xs = values.filter((v) => Number.isFinite(v));
  return xs.length ? xs.reduce((s, v) => s + v, 0) / xs.length : 0;
}

/**
 * Het p-kwantiel met lineaire interpolatie tussen de omliggende waarnemingen,
 * zoals numpy.quantile het standaard doet — de POC's grenzen komen daarvandaan.
 */
export function quantile(sorted, p) {
  if (!sorted.length) return 0;
  if (sorted.length === 1) return sorted[0];
  const pos = (sorted.length - 1) * Math.min(1, Math.max(0, p));
  const lower = Math.floor(pos);
  const upper = Math.ceil(pos);
  if (lower === upper) return sorted[lower];
  return sorted[lower] + (pos - lower) * (sorted[upper] - sorted[lower]);
}

/**
 * Eta-kwadraat: welk deel van de spreiding in de bedragen wordt verklaard
 * doordat je de mensen in groepen deelt. 0 betekent dat de groep niets zegt,
 * 1 dat de groep alles bepaalt. Hiermee wordt gekozen wélke kenmerken een
 * eigen staffel verdienen.
 */
export function etaSquared(groups, overallMean, ssTotal) {
  if (!ssTotal) return 0;
  let ssBetween = 0;
  for (const values of groups) {
    if (!values.length) continue;
    const m = mean(values);
    ssBetween += values.length * (m - overallMean) ** 2;
  }
  return ssBetween / ssTotal;
}

function sumSquares(values, m) {
  return values.reduce((s, v) => s + (v - m) ** 2, 0);
}

// ---- kenmerken uit een simulatierun ----------------------------------------

/**
 * De kenmerken waarop geharmoniseerd kan worden, per publiek. Dit zijn de
 * eigenschappen die de bevolkingsgenerator van elk subject vastlegt
 * (`simulation/population.js`), vertaald naar de vorm die het model gebruikt:
 * een getal om op te staffelen, of een ja/nee om op te groeperen.
 */
export const CITIZEN_FEATURES = [
  { key: 'inkomen', label: 'Inkomen', kind: 'number', of: (s) => s.inkomen ?? 0 },
  { key: 'leeftijd', label: 'Leeftijd', kind: 'number', of: (s) => s.leeftijd ?? 0 },
  { key: 'huur', label: 'Huur per maand', kind: 'number', of: (s) => s.huur ?? 0 },
  { key: 'kinderen', label: 'Aantal kinderen', kind: 'number', of: (s) => s.kinderen ?? 0 },
  { key: 'heeft_partner', label: 'Heeft partner', kind: 'boolean', of: (s) => (s.partner ? 1 : 0) },
  { key: 'heeft_kinderen', label: 'Heeft kinderen', kind: 'boolean', of: (s) => ((s.kinderen ?? 0) > 0 ? 1 : 0) },
  { key: 'huurder', label: 'Huurt een woning', kind: 'boolean', of: (s) => (s.huurder ? 1 : 0) },
  { key: 'student', label: 'Is student', kind: 'boolean', of: (s) => (s.student ? 1 : 0) },
];

export const BUSINESS_FEATURES = [
  { key: 'oppervlakte', label: 'Vloeroppervlakte', kind: 'number', of: (s) => s.oppervlakte ?? 0 },
  { key: 'werknemers', label: 'Aantal werknemers', kind: 'number', of: (s) => s.werknemers ?? 0 },
  { key: 'horeca', label: 'Is horeca', kind: 'boolean', of: (s) => (s.type && s.type !== 'overig' ? 1 : 0) },
  { key: 'voedsel', label: 'Bereidt voedsel', kind: 'boolean', of: (s) => (s.voedsel ? 1 : 0) },
  { key: 'terras', label: 'Heeft terras', kind: 'boolean', of: (s) => (s.terras ? 1 : 0) },
];

export function featuresFor(kind) {
  return kind === 'ondernemers' ? BUSINESS_FEATURES : CITIZEN_FEATURES;
}

/**
 * Welke van de gekozen wetten geld kósten in plaats van opleveren.
 *
 * Een belasting en een toeslag bij elkaar optellen alsof ze hetzelfde teken
 * hebben levert een onzinnig bedrag op: € 4.000 belasting plus € 1.600
 * zorgtoeslag is niet € 5.600 waar iemand recht op heeft. Welke wet een
 * belasting is staat in `simulation.disposable_income` in demo-config.yaml
 * (`kind: tax`), dezelfde bron die de besteedbaar-inkomenberekening gebruikt;
 * de wet zelf zegt het niet.
 */
export function taxLawIds(corpus) {
  const components = corpus?.config?.simulation?.disposable_income ?? [];
  return new Set(components.filter((c) => c.kind === 'tax').map((c) => c.law));
}

/**
 * Een simulatierun omzetten naar de tabel waarop geleerd wordt: per subject de
 * kenmerken, of er recht bestond op één van de gekozen wetten, en het totaal
 * van wat die wetten netto opleverden.
 *
 * Meerdere wetten optellen is precies de vraag die harmonisatie stelt: wat de
 * burger overhoudt is de som, niet de losse regelingen. Een belasting telt
 * daarin negatief mee.
 *
 * @param {object} run        een run van `runSimulation`
 * @param {string[]} lawIds   de wetten die samengenomen worden
 * @param {Set<string>} [taxes]  wetten die geld kosten (zie `taxLawIds`)
 */
export function trainingData(run, lawIds, taxes = new Set()) {
  const features = featuresFor(run.kind);
  const rows = [];
  for (const result of run.results) {
    const values = {};
    for (const f of features) values[f.key] = f.of(result.subject);
    let eligible = false;
    let amount = 0;
    let usable = false;
    for (const id of lawIds) {
      const law = result.laws[id];
      if (!law?.ok) continue;
      usable = true;
      // Een wet zonder voorwaardenuitvoer (een belasting) geldt voor iedereen;
      // 'onbekend' is geen ja.
      if (law.met === true || law.met === null) eligible = true;
      if (typeof law.amount === 'number') amount += taxes.has(id) ? -law.amount : law.amount;
    }
    // Een subject waarvoor geen enkele gekozen wet kon rekenen zegt niets over
    // de vorm van de regeling en zou het gemiddelde vertekenen.
    if (!usable) continue;
    rows.push({ values, eligible, amount, subject: result.subject });
  }
  return { rows, features };
}

// ---- staffelmodel ----------------------------------------------------------

const DEFAULTS = {
  brackets: 5,
  /** Bedragen afronden op hele euro's: een staffel met centen leest niemand. */
  roundAmount: 1,
  minGroupSize: 30,
  maxGroupKeys: 2,
  /** Onder deze verklaarde spreiding is een eigen staffel de complexiteit niet waard. */
  minEta: 0.05,
};

function roundTo(value, step) {
  return step > 0 ? Math.round(value / step) * step : value;
}

/**
 * De grens van elke staffeltrede: kwantielen van het gekozen kenmerk, afgerond
 * op een leesbaar getal. Een grens van € 31.284 is geen wet; € 31.000 wel.
 */
function bracketBoundaries(values, count) {
  const sorted = [...values].sort((a, b) => a - b);
  const span = (sorted[sorted.length - 1] ?? 0) - (sorted[0] ?? 0);
  // De afronding volgt de schaal: leeftijd op jaren, inkomen op duizendtallen.
  const step = span > 20000 ? 1000 : span > 2000 ? 100 : span > 200 ? 10 : 1;
  const raw = [];
  for (let i = 0; i <= count; i += 1) raw.push(quantile(sorted, i / count));
  const rounded = [...new Set(raw.map((b) => roundTo(b, step)))].sort((a, b) => a - b);
  return rounded.length >= 2 ? rounded : [sorted[0] ?? 0, (sorted[sorted.length - 1] ?? 0) + 1];
}

/**
 * Kleinste kwadraten binnen één trede, geëvalueerd op de beide grenzen.
 *
 * Niet afgekapt op nul: als er een belasting in de som zit is het nettobedrag
 * negatief, en dat is een echte uitkomst — iemand die per saldo betaalt, niet
 * iemand die niets krijgt.
 */
function fitSegment(points, lower, upper) {
  if (!points.length) return [0, 0];
  if (points.length < 3) {
    const m = mean(points.map((p) => p.amount));
    return [m, m];
  }
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.amount);
  const mx = mean(xs);
  const my = mean(ys);
  let num = 0;
  let den = 0;
  for (let i = 0; i < xs.length; i += 1) {
    num += (xs[i] - mx) * (ys[i] - my);
    den += (xs[i] - mx) ** 2;
  }
  const slope = den > 0 ? num / den : 0;
  const at = (x) => my + slope * (x - mx);
  return [at(lower), at(upper)];
}

/**
 * Een staffelmodel leren.
 *
 * Eerst wordt gekozen waarop gestaffeld wordt (het kenmerk met de meeste
 * invloed op het bedrag, standaard het inkomen), daarna welke ja/nee-kenmerken
 * een eigen staffel krijgen, en ten slotte per groep het bedrag op elke
 * trederand. Tussen twee randen loopt het bedrag rechtlijnig, dus er zit nooit
 * een sprong in: precies één euro meer verdienen kost nooit honderd euro
 * toeslag.
 *
 * @param {{rows: Array, features: Array}} data  uit `trainingData`
 * @param {object} [options]
 */
export function trainBracketModel(data, options = {}) {
  const config = { ...DEFAULTS, ...options };
  const { rows, features } = data;
  if (rows.length < 10) throw new Error('Te weinig gegevens om een model te leren; draai een grotere simulatie.');

  const amounts = rows.map((r) => r.amount);
  const overallMean = mean(amounts);
  const ssTotal = sumSquares(amounts, overallMean);

  // Welke kenmerken doen ertoe? Voor een getal wordt de beste tweedeling
  // gezocht, voor een ja/nee is de deling al gegeven.
  const influence = [];
  for (const f of features) {
    const values = rows.map((r) => r.values[f.key]);
    if (f.kind === 'boolean') {
      const yes = rows.filter((r) => r.values[f.key] === 1).map((r) => r.amount);
      const no = rows.filter((r) => r.values[f.key] !== 1).map((r) => r.amount);
      if (yes.length < config.minGroupSize || no.length < config.minGroupSize) continue;
      influence.push({ feature: f, eta: etaSquared([yes, no], overallMean, ssTotal), threshold: null });
    } else {
      const { threshold, eta } = bestSplit(rows, f.key, overallMean, ssTotal, config.minGroupSize);
      influence.push({ feature: f, eta, threshold });
    }
  }
  influence.sort((a, b) => b.eta - a.eta);

  // Waarop staffelen: het opgegeven kenmerk, anders het invloedrijkste getal.
  const primary =
    features.find((f) => f.key === options.primary && f.kind === 'number') ??
    influence.find((i) => i.feature.kind === 'number')?.feature ??
    features.find((f) => f.kind === 'number');
  if (!primary) throw new Error('Geen numeriek kenmerk om op te staffelen.');

  // Waarop groeperen: de invloedrijkste ja/nee-kenmerken, behalve het kenmerk
  // waarop al gestaffeld wordt.
  const groupKeys = influence
    .filter((i) => i.feature.kind === 'boolean' && i.feature.key !== primary.key && i.eta >= config.minEta)
    .slice(0, config.maxGroupKeys)
    .map((i) => i.feature);

  const boundaries = bracketBoundaries(rows.map((r) => r.values[primary.key]), config.brackets);

  // Elke combinatie van de groepskenmerken krijgt een eigen staffel, mits er
  // genoeg mensen in zitten om iets zinnigs over te zeggen.
  const combos = groupCombinations(groupKeys);
  const groups = [];
  for (const combo of combos) {
    const members = rows.filter((r) => groupKeys.every((f) => r.values[f.key] === combo[f.key]));
    if (members.length < Math.max(config.minGroupSize, 5 * (boundaries.length - 1))) continue;
    const points = members.map((r) => ({ x: r.values[primary.key], amount: r.amount }));
    const steps = [];
    for (let i = 0; i < boundaries.length - 1; i += 1) {
      const lower = boundaries[i];
      const upper = boundaries[i + 1];
      const inBracket = points.filter((p) => p.x >= lower && p.x <= upper);
      const [atLower, atUpper] = fitSegment(inBracket.length >= 3 ? inBracket : points, lower, upper);
      steps.push({
        lower,
        upper,
        amountAtLower: roundTo(atLower, config.roundAmount),
        amountAtUpper: roundTo(atUpper, config.roundAmount),
        count: inBracket.length,
      });
    }
    groups.push({ filter: combo, keys: groupKeys.map((f) => f.key), steps, count: members.length });
  }

  // Zonder bruikbare groepen: één staffel voor iedereen.
  if (!groups.length) {
    const points = rows.map((r) => ({ x: r.values[primary.key], amount: r.amount }));
    const steps = [];
    for (let i = 0; i < boundaries.length - 1; i += 1) {
      const inBracket = points.filter((p) => p.x >= boundaries[i] && p.x <= boundaries[i + 1]);
      const [atLower, atUpper] = fitSegment(inBracket.length >= 3 ? inBracket : points, boundaries[i], boundaries[i + 1]);
      steps.push({
        lower: boundaries[i],
        upper: boundaries[i + 1],
        amountAtLower: roundTo(atLower, config.roundAmount),
        amountAtUpper: roundTo(atUpper, config.roundAmount),
        count: inBracket.length,
      });
    }
    groups.push({ filter: {}, keys: [], steps, count: rows.length });
  }

  const model = { primary, groupKeys, boundaries, groups, influence, config };
  return { ...model, metrics: evaluateModel(model, rows) };
}

/** De beste tweedeling van een numeriek kenmerk, op verklaarde spreiding. */
function bestSplit(rows, key, overallMean, ssTotal, minGroupSize) {
  const values = rows.map((r) => r.values[key]);
  const sorted = [...values].sort((a, b) => a - b);
  const span = sorted[sorted.length - 1] - sorted[0];
  const step = span > 20000 ? 1000 : span > 2000 ? 100 : span > 200 ? 10 : 1;
  const candidates = new Set();
  for (let p = 0.1; p <= 0.9; p += 0.1) candidates.add(roundTo(quantile(sorted, p), step));
  let best = { threshold: null, eta: 0 };
  for (const t of candidates) {
    const below = rows.filter((r) => r.values[key] <= t).map((r) => r.amount);
    const above = rows.filter((r) => r.values[key] > t).map((r) => r.amount);
    if (below.length < minGroupSize || above.length < minGroupSize) continue;
    const eta = etaSquared([below, above], overallMean, ssTotal);
    if (eta > best.eta) best = { threshold: t, eta };
  }
  return best;
}

/** Alle combinaties van ja/nee over de groepskenmerken. */
function groupCombinations(keys) {
  let combos = [{}];
  for (const f of keys) {
    const next = [];
    for (const c of combos) {
      next.push({ ...c, [f.key]: 0 });
      next.push({ ...c, [f.key]: 1 });
    }
    combos = next;
  }
  return combos;
}

/**
 * Wat het model voor één persoon zou toekennen.
 *
 * Elke trede wordt apart gefit, dus het bedrag op de bovengrens van de ene
 * trede hoeft niet tot op de cent gelijk te zijn aan dat op de ondergrens van
 * de volgende. Gemeten op een sterk gebogen functie blijft dat verschil binnen
 * de afronding op hele euro's (hooguit € 2), dus als sprong is het niet
 * zichtbaar; op de grens zelf wint de eerste trede die past.
 */
export function predict(model, values) {
  const group =
    model.groups.find((g) => g.keys.every((k) => values[k] === g.filter[k])) ??
    model.groups.find((g) => g.keys.length === 0) ??
    model.groups[0];
  if (!group) return 0;
  const x = values[model.primary.key] ?? 0;
  const steps = group.steps;
  if (!steps.length) return 0;
  if (x <= steps[0].lower) return steps[0].amountAtLower;
  const last = steps[steps.length - 1];
  if (x >= last.upper) return last.amountAtUpper;
  for (const s of steps) {
    if (x >= s.lower && x <= s.upper) {
      const width = s.upper - s.lower;
      if (width === 0) return s.amountAtLower;
      const t = (x - s.lower) / width;
      return s.amountAtLower + t * (s.amountAtUpper - s.amountAtLower);
    }
  }
  return last.amountAtUpper;
}

/**
 * Hoe goed benadert het model de bestaande wetten?
 *
 * R² is het deel van de spreiding dat het model verklaart; MAE het gemiddelde
 * verschil in euro's. Die tweede is de eerlijkste: een R² van 0,9 klinkt goed,
 * maar € 40 per maand ernaast is voor wie het aangaat gewoon € 40.
 */
export function evaluateModel(model, rows) {
  if (!rows.length) return { r2: 0, mae: 0, meanAmount: 0, worst: [] };
  const errors = [];
  let ssResidual = 0;
  const actual = rows.map((r) => r.amount);
  const m = mean(actual);
  for (const row of rows) {
    const predicted = predict(model, row.values);
    const error = predicted - row.amount;
    ssResidual += error ** 2;
    errors.push({ ...row, predicted, error });
  }
  const ssTotal = sumSquares(actual, m);
  errors.sort((a, b) => Math.abs(b.error) - Math.abs(a.error));
  return {
    r2: ssTotal > 0 ? 1 - ssResidual / ssTotal : 0,
    mae: mean(errors.map((e) => Math.abs(e.error))),
    meanAmount: m,
    // Waar het model er het verst naast zit: daar zit de regel die de staffel
    // niet kan volgen, en dat is het interessantste deel van de uitkomst.
    worst: errors.slice(0, 5).map((e) => ({ values: e.values, actual: e.amount, predicted: e.predicted, error: e.error })),
  };
}

/**
 * Het model als leesbare regeling, in de vorm waarin een wet het zou zeggen.
 * Geen YAML zoals de POC genereerde — die was bedoeld om weer uit te voeren;
 * dit is bedoeld om te lezen en te bespreken.
 */
export function describeModel(model, formatAmount = (v) => `€ ${v.toFixed(0)}`) {
  const lines = [];
  for (const group of model.groups) {
    const label = group.keys.length
      ? group.keys.map((k) => `${labelOf(model, k)}: ${group.filter[k] ? 'ja' : 'nee'}`).join(', ')
      : 'Iedereen';
    lines.push({
      group: label,
      count: group.count,
      steps: group.steps.map((s) => ({
        range: `${formatNumber(s.lower)} – ${formatNumber(s.upper)}`,
        from: formatAmount(s.amountAtLower),
        to: formatAmount(s.amountAtUpper),
        count: s.count,
      })),
    });
  }
  return lines;
}

function labelOf(model, key) {
  return model.groupKeys.find((f) => f.key === key)?.label ?? key;
}

function formatNumber(v) {
  return new Intl.NumberFormat('nl-NL', { maximumFractionDigits: 0 }).format(v);
}
