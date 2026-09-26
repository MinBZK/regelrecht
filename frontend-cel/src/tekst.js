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

// Een uitkomst als tekst, naar haar type uit de regeling (`{type, eenheid}`,
// zoals de runtime het per uitkomst meegeeft): een bedrag in zijn eenheid,
// de rest als waarde.
export function uitkomstTekst(w, type) {
  return type?.type === 'amount' ? bedragTekst(w, type.eenheid) : waardeTekst(w);
}

// Waar een parameter vandaan kwam: één tekst per variant van Herkomst in
// packages/cel/src/synthese.rs (de tests in tekst.test.js lopen ze alle na).
export function herkomstTekst(h) {
  switch (h?.bron) {
    case 'eigen':
      return `eigen lexostatus ${h.lexostatus}`;
    case 'cel':
      return `cel ${h.cel}, lexostatus ${h.lexostatus} (${h.transport})`;
    case 'per_regel':
      return `per regel uit ${h.veld} van eigen lexostatus ${h.lexostatus}`;
    case 'behandelaar':
      return 'behandelaar (formulier van de handeling)';
    case 'stand_bij_besluit':
      return h.stage ? `stand bij besluit (ontstaat pas in stage ${h.stage})` : 'stand bij besluit';
    case 'keuze':
      return 'keuze van de aanvrager (portaal)';
    default:
      // Een variant die de runtime kent en deze tekst nog niet: laat zien wat
      // er binnenkwam in plaats van niets.
      return JSON.stringify(h);
  }
}

// De parameters die naar de engine gingen, met waarde en herkomst.
export function herkomstRijen(parameters, herkomst) {
  return Object.entries(herkomst ?? {}).map(([naam, bron]) => ({
    naam,
    waarde: waardeTekst((parameters ?? {})[naam]),
    bron: herkomstTekst(bron),
  }));
}

// De soort van een handeling: de runtime geeft haar als {soort, ...}.
export function soortVan(h) {
  return typeof h?.soort === 'object' && h.soort !== null ? h.soort.soort : h?.soort;
}
