// Kanalen en rollen zoals GET /api/processen ze van een proces geeft
// (`kanalen` en `rollen` in proces.yaml). Geen kanaal staat hier vast: de
// schermen bouwen hun velden, labels en keuzes uit deze beschrijving.

// De rollen van een proces als lijst: [{id, label, kanaal, routes}], in de
// volgorde van de runtime.
export function rollenVan(proces) {
  return Object.entries(proces?.rollen ?? {}).map(([id, r]) => ({ id, ...r }));
}

// Het eerste scherm van een rol: wat haar eerste routegroep laat zien. Een
// rol zonder scherm heeft geen beginscherm (null): de kroniek van de cel is
// niet open, alleen een behandelaar ziet haar.
export function beginscherm(proces, rol) {
  const routes = proces?.rollen?.[rol]?.routes ?? [];
  if (routes.includes('portaal') && proces.portaal) return 'mogelijkheden';
  if (routes.includes('behandeling') && proces.behandeling) return 'werkvoorraad';
  if (routes.includes('loket') && proces.loket) return 'loket';
  return null;
}

// De kanalen waarlangs het portaal inlogt: die van de rollen met routes
// `portaal`, elk een keer, als [{id, ...kanaal}]. Het loket duidt de
// aanvrager aan met de velden van zo'n kanaal.
// `rol` is het label van de eerste portaalrol van het kanaal: zo noemt het
// loket de keuze.
export function portaalkanalen(proces) {
  const uit = [];
  for (const r of rollenVan(proces)) {
    const k = proces.kanalen?.[r.kanaal];
    if (r.routes.includes('portaal') && k && !uit.some((x) => x.id === r.kanaal)) {
      uit.push({ id: r.kanaal, rol: r.label ?? r.id, ...k });
    }
  }
  return uit;
}

// Wie er is ingelogd, als tekst: de waarden van de velden in de volgorde
// van het kanaal, en het label van de rol.
export function sessieTekst(proces, sessie) {
  if (!sessie) return '';
  const velden = proces?.kanalen?.[sessie.kanaal]?.velden ?? [];
  const waarden = velden.map((v) => sessie.velden?.[v.naam]).filter(Boolean);
  const rol = proces?.rollen?.[sessie.rol]?.label ?? sessie.rol;
  return [...waarden, rol].join(', ');
}

// Een waarde per veld van een kanaal, leeg of uit een voorbeeld.
export function lege(kanaal, bron = {}) {
  return Object.fromEntries((kanaal?.velden ?? []).map((v) => [v.naam, bron[v.naam] ?? '']));
}
