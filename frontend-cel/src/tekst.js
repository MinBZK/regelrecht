// Waarden en herkomst als tekst, voor tabellen.

// Een waarde uit een lexostatus of een uitkomst van de engine.
export function waardeTekst(w) {
  if (w === true) return 'ja';
  if (w === false) return 'nee';
  if (w === undefined) return '';
  if (w === null) return 'geen (null)';
  return typeof w === 'string' ? w : JSON.stringify(w);
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
      return 'behandelaar (besluitformulier)';
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
