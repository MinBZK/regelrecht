// Waarden en herkomst als tekst, voor tabellen.
import { centsToEuros } from '@regelrecht/frontend-shared/currency.js';
import { isUnknown, missingFacts } from '@regelrecht/frontend-shared/values.js';

// Een waarde uit een lexostatus of een uitkomst van de engine. Een onbekende
// waarde (RFC-036) noemt wat er mist.
export function waardeTekst(w) {
  if (w === true) return 'ja';
  if (w === false) return 'nee';
  if (w === undefined) return '';
  if (w === null) return 'geen (null)';
  if (isUnknown(w)) {
    const mist = missingFacts(w).map((f) => f.name);
    return mist.length > 0 ? `onbekend (mist ${mist.join(', ')})` : 'onbekend';
  }
  return typeof w === 'string' ? w : JSON.stringify(w);
}

const euro = new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR' });

// Een bedrag in de eenheid die de regeling noemt (`type_spec.unit`): eurocent
// en euro als euro's, zoals een Nederlands overheidsscherm het noteert
// ("€ 19.136,00"); een andere eenheid achter het getal, en zonder eenheid
// het getal zelf.
export function bedragTekst(w, eenheid) {
  if (typeof w !== 'number') return waardeTekst(w);
  if (eenheid === 'eurocent') return euro.format(centsToEuros(w));
  if (eenheid === 'euro') return euro.format(w);
  return eenheid ? `${w} ${eenheid}` : String(w);
}

// Een uitkomst als tekst, naar haar type uit de regeling (`{type, unit}`,
// zoals de runtime het per uitkomst meegeeft): een bedrag in zijn eenheid,
// de rest als waarde.
export function uitkomstTekst(w, type) {
  return type?.type === 'amount' ? bedragTekst(w, type.unit) : waardeTekst(w);
}

// Waar een parameter vandaan kwam: één tekst per variant van Herkomst in
// packages/cel/src/synthese.rs (de tests in tekst.test.js lopen ze alle na).
export function herkomstTekst(h) {
  switch (h?.source) {
    case 'own':
      return `eigen lexostatus ${h.lexostatus}`;
    case 'cell':
      return `cel ${h.cell}, lexostatus ${h.lexostatus} (${h.transport})`;
    case 'per_row':
      return `per regel uit ${h.field} van eigen lexostatus ${h.lexostatus}`;
    case 'handler':
      return 'behandelaar (formulier van de handeling)';
    case 'state_at_decision':
      return h.stage ? `stand bij besluit (ontstaat pas in stage ${h.stage})` : 'stand bij besluit';
    case 'choice':
      return 'keuze van de aanvrager (portaal)';
    default:
      // Een variant die de runtime kent en deze tekst nog niet: laat zien wat
      // er binnenkwam in plaats van niets.
      return JSON.stringify(h);
  }
}

// Langs welke route een cel een lexostatus reduceerde (experiment A, alleen
// in een runtime met CELL_REDUCTION): `{route, regulation, reason, duration_us}` als
// tekst; leeg zonder route.
export function routeTekst(r) {
  if (!r?.route) return '';
  const duur = typeof r.duration_us === 'number' ? `, ${(r.duration_us / 1000).toFixed(2)} ms` : '';
  if (r.route === 'engine') return `reductie via de engine (${r.regulation}${duur})`;
  if (r.route === 'runtime') return `door de runtime zelf${r.reason ? ` (${r.reason})` : ''}`;
  return `reductie via de DSL${r.reason ? ` (bewust: ${r.reason})` : ''}${duur}`;
}

// De route per lexostatus uit een antwoord van een toets of een handeling:
// de eigen lexostatussen (`lexostatuses`, bij de toets `lexostatus`) en de
// bronnen van de synthese (`sources`, met hun cel). Een functie (bron uit de
// herkomst) -> route.
export function routesUit(antwoord) {
  const lexostatussen = antwoord?.lexostatuses ?? (antwoord?.lexostatus ? [antwoord.lexostatus] : []);
  const eigen = new Map(lexostatussen.map((l) => [l.name, l.reduction]));
  const bronnen = new Map((antwoord?.sources ?? []).map((b) => [`${b.cell}/${b.lexostatus}`, b.reduction]));
  return (h) => {
    if (h?.source === 'own' || h?.source === 'per_row') return eigen.get(h.lexostatus) ?? null;
    if (h?.source === 'cell') return bronnen.get(`${h.cell}/${h.lexostatus}`) ?? null;
    return null;
  };
}

// De parameters die naar de engine gingen, met waarde en herkomst; met
// `routeVan` (zie routesUit) ook langs welke route de lexostatus kwam.
export function herkomstRijen(parameters, herkomst, routeVan = () => null) {
  return Object.entries(herkomst ?? {}).map(([naam, bron]) => {
    const route = routeTekst(routeVan(bron));
    return {
      naam,
      waarde: waardeTekst((parameters ?? {})[naam]),
      bron: route ? `${herkomstTekst(bron)}; ${route}` : herkomstTekst(bron),
    };
  });
}

// De soort van een handeling: de runtime geeft haar als {kind, ...}, of
// (bij een proef) als `kind` naast de rest.
export function soortVan(h) {
  return typeof h?.kind === 'object' && h.kind !== null ? h.kind.kind : h?.kind;
}

// Hoe de vraag aan een bron verliep, in woorden (`status` van een bron in
// packages/cel/src/synthese.rs).
const BRONSTATUS = {
  queried: 'bevraagd',
  unreachable: 'onbereikbaar',
  error: 'fout',
  not_queried: 'niet bevraagd',
};

export function bronStatusTekst(s) {
  return BRONSTATUS[s] ?? String(s ?? '').replace(/_/g, ' ');
}
