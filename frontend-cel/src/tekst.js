// Waarden en herkomst als tekst, voor tabellen.

// Een waarde uit een lexostatus of een uitkomst van de engine.
export function waardeTekst(w) {
  if (w === true) return 'ja';
  if (w === false) return 'nee';
  if (w === undefined) return '';
  if (w === null) return 'geen (null)';
  return typeof w === 'string' ? w : JSON.stringify(w);
}

// Een bedrag in eurocent als euro's, zoals een Nederlands overheidsscherm
// het noteert: "€ 19.136,00".
export function euroTekst(centen) {
  if (typeof centen !== 'number') return waardeTekst(centen);
  return new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR' }).format(centen / 100);
}

// Of een uitkomst een bedrag is. De engine geeft geen type mee in de proef;
// een bedrag herkennen we aan zijn naam (de corpora rekenen bedragen in
// eurocent).
export function isBedrag(naam) {
  return /bedrag|te_betalen|betaald/.test(naam);
}

// Een uitkomst als tekst: een bedrag in euro, de rest als waarde.
export function uitkomstTekst(naam, w) {
  return isBedrag(naam) && typeof w === 'number' ? euroTekst(w) : waardeTekst(w);
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
