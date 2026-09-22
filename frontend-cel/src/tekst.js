// Waarden en herkomst als tekst, voor tabellen.

// Een waarde uit een lexostatus of een uitkomst van de engine.
export function waardeTekst(w) {
  if (w === true) return 'ja';
  if (w === false) return 'nee';
  if (w === undefined) return '';
  if (w === null) return 'geen (null)';
  return typeof w === 'string' ? w : JSON.stringify(w);
}

// Waar een parameter vandaan kwam (zie Herkomst in packages/cel/src/synthese.rs).
export function herkomstTekst(h) {
  switch (h?.bron) {
    case 'eigen':
      return `eigen lexostatus ${h.lexostatus}`;
    case 'cel':
      return `cel ${h.cel}, lexostatus ${h.lexostatus} (${h.transport})`;
    case 'behandelaar':
      return 'behandelaar (besluitformulier)';
    case 'stand_bij_besluit':
      return 'stand bij besluit';
    default:
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
