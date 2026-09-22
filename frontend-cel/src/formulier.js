// Van wat er in het formulier staat naar `external` bij het indienen.
//
// Een leeg veld gaat niet mee: de cel legt het dan vast als null, en een
// afleiding als `gevuld` ziet het als niet ingevuld. Een naam met een punt
// (`a.b`) wordt genest, zoals `$external.a.b` in de stroom.

export function leeg(waarde) {
  if (waarde === undefined || waarde === null) return true;
  if (typeof waarde === 'string') return waarde.trim() === '';
  if (Array.isArray(waarde)) return waarde.length === 0;
  return false;
}

function regel(r) {
  const uit = {};
  for (const [k, v] of Object.entries(r)) {
    if (!leeg(v)) uit[k] = v;
  }
  return uit;
}

export function external(waarden) {
  const uit = {};
  for (const [naam, waarde] of Object.entries(waarden)) {
    let w = waarde;
    if (Array.isArray(w)) w = w.map(regel).filter((r) => Object.keys(r).length > 0);
    if (leeg(w)) continue;
    const delen = naam.split('.');
    let doel = uit;
    for (const deel of delen.slice(0, -1)) doel = doel[deel] ??= {};
    doel[delen.at(-1)] = w;
  }
  return uit;
}

// Opties uit een formulier zijn teksten of {waarde, label}.
export function opties(lijst) {
  return (lijst ?? []).map((o) =>
    typeof o === 'object' && o !== null
      ? { waarde: o.waarde ?? o.value, label: o.label ?? String(o.waarde ?? o.value) }
      : { waarde: o, label: String(o) },
  );
}
