// Kanalen en rollen zoals GET /api/processes ze van een proces geeft
// (`channels` en `roles` in process.yaml). Geen kanaal staat hier vast: de
// schermen bouwen hun velden, labels en keuzes uit deze beschrijving.

// De rollen van een proces als lijst: [{id, label, channel, routes}], in de
// volgorde van de runtime.
export function rollenVan(proces) {
  return Object.entries(proces?.roles ?? {}).map(([id, r]) => ({ id, ...r }));
}

// Het eerste scherm van een rol: wat haar eerste routegroep laat zien. Een
// rol zonder scherm heeft geen beginscherm (null): de kroniek van de cel is
// niet open, alleen een behandelaar ziet haar.
export function beginscherm(proces, rol) {
  const routes = proces?.roles?.[rol]?.routes ?? [];
  if (routes.includes('portal') && proces.portal) return 'mogelijkheden';
  if (routes.includes('handling') && proces.handling) return 'werkvoorraad';
  if (routes.includes('counter') && proces.counter) return 'loket';
  return null;
}

// De kanalen waarlangs het portaal inlogt: die van de rollen met routes
// `portal`, elk een keer, als [{id, ...kanaal}]. Het loket duidt de
// aanvrager aan met de velden van zo'n kanaal.
// `rol` is het label van de eerste portaalrol van het kanaal: zo noemt het
// loket de keuze.
export function portaalkanalen(proces) {
  const uit = [];
  for (const r of rollenVan(proces)) {
    const k = proces.channels?.[r.channel];
    if (r.routes.includes('portal') && k && !uit.some((x) => x.id === r.channel)) {
      uit.push({ id: r.channel, rol: r.label ?? r.id, ...k });
    }
  }
  return uit;
}

// Wie er is ingelogd, als tekst: de waarden van de velden in de volgorde
// van het kanaal, en het label van de rol.
export function sessieTekst(proces, sessie) {
  if (!sessie) return '';
  const velden = proces?.channels?.[sessie.channel]?.fields ?? [];
  const waarden = velden.map((v) => sessie.fields?.[v.name]).filter(Boolean);
  const rol = proces?.roles?.[sessie.role]?.label ?? sessie.role;
  return [...waarden, rol].join(', ');
}

// Een waarde per veld van een kanaal, leeg of uit een voorbeeld.
export function lege(kanaal, bron = {}) {
  return Object.fromEntries((kanaal?.fields ?? []).map((v) => [v.name, bron[v.name] ?? '']));
}
