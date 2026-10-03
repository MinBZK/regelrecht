// Values and provenance as text, for tables. The texts themselves are Dutch:
// the user sees them.
import { centsToEuros } from '@regelrecht/frontend-shared/currency.js';
import { isUnknown, missingFacts } from '@regelrecht/frontend-shared/values.js';

// A value from a lexostatus or an output of the engine. An unknown value
// (RFC-036) names what is missing.
export function valueText(v) {
  if (v === true) return 'ja';
  if (v === false) return 'nee';
  if (v === undefined) return '';
  if (v === null) return 'geen (null)';
  if (isUnknown(v)) {
    const missing = missingFacts(v).map((f) => f.name);
    return missing.length > 0 ? `onbekend (mist ${missing.join(', ')})` : 'onbekend';
  }
  return typeof v === 'string' ? v : JSON.stringify(v);
}

const euro = new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR' });

// An amount in the unit the regulation names (`type_spec.unit`): eurocents
// and euros as euros, the way a Dutch government screen writes it
// ("€ 19.136,00"); another unit after the number, and without a unit the
// number itself.
export function amountText(v, unit) {
  if (typeof v !== 'number') return valueText(v);
  if (unit === 'eurocent') return euro.format(centsToEuros(v));
  if (unit === 'euro') return euro.format(v);
  return unit ? `${v} ${unit}` : String(v);
}

// An output as text, by its type from the regulation (`{type, unit}`, as the
// runtime gives it per output): an amount in its unit, the rest as a value.
export function outputText(v, type) {
  return type?.type === 'amount' ? amountText(v, type.unit) : valueText(v);
}

// Where a parameter came from: one text per variant of Herkomst in
// packages/cel/src/synthese.rs (the tests in text.test.js cover them all).
export function provenanceText(p) {
  switch (p?.source) {
    case 'own':
      return `eigen lexostatus ${p.lexostatus}`;
    case 'cell':
      return `cel ${p.cell}, lexostatus ${p.lexostatus} (${p.transport})`;
    case 'per_row':
      return `per regel uit ${p.field} van eigen lexostatus ${p.lexostatus}`;
    case 'handler':
      return 'behandelaar (formulier van de handeling)';
    case 'state_at_decision':
      return p.stage ? `stand bij besluit (ontstaat pas in stage ${p.stage})` : 'stand bij besluit';
    case 'choice':
      return 'keuze van de aanvrager (portaal)';
    default:
      // A variant the runtime knows and this text does not yet: show what
      // came in instead of nothing.
      return JSON.stringify(p);
  }
}

// Along which route a cell reduced a lexostatus (experiment A, only in a
// runtime with CELL_REDUCTION): `{route, regulation, reason, duration_us}` as
// text; empty without a route.
export function routeText(r) {
  if (!r?.route) return '';
  const duration = typeof r.duration_us === 'number' ? `, ${(r.duration_us / 1000).toFixed(2)} ms` : '';
  if (r.route === 'engine') return `reductie via de engine (${r.regulation}${duration})`;
  if (r.route === 'runtime') return `door de runtime zelf${r.reason ? ` (${r.reason})` : ''}`;
  return `reductie via de DSL${r.reason ? ` (bewust: ${r.reason})` : ''}${duration}`;
}

// The route per lexostatus from the response of an assessment or an action:
// the own lexostatuses (`lexostatuses`, for the assessment `lexostatus`) and
// the sources of the synthesis (`sources`, with their cell). Returns a
// function (source from the provenance) -> route.
export function routesFrom(response) {
  const lexostatuses = response?.lexostatuses ?? (response?.lexostatus ? [response.lexostatus] : []);
  const own = new Map(lexostatuses.map((l) => [l.name, l.reduction]));
  const sources = new Map((response?.sources ?? []).map((s) => [`${s.cell}/${s.lexostatus}`, s.reduction]));
  return (p) => {
    if (p?.source === 'own' || p?.source === 'per_row') return own.get(p.lexostatus) ?? null;
    if (p?.source === 'cell') return sources.get(`${p.cell}/${p.lexostatus}`) ?? null;
    return null;
  };
}

// The parameters that went to the engine, with value and provenance; with
// `routeOf` (see routesFrom) also along which route the lexostatus came.
export function provenanceRows(parameters, provenance, routeOf = () => null) {
  return Object.entries(provenance ?? {}).map(([name, p]) => {
    const route = routeText(routeOf(p));
    return {
      name,
      value: valueText((parameters ?? {})[name]),
      source: route ? `${provenanceText(p)}; ${route}` : provenanceText(p),
    };
  });
}

// The kind of an action: the runtime gives it as {kind, ...}, or (on a
// trial) as `kind` next to the rest.
export function kindOf(action) {
  return typeof action?.kind === 'object' && action.kind !== null ? action.kind.kind : action?.kind;
}

// How querying a source went, in words (`status` of a source in
// packages/cel/src/synthese.rs).
const SOURCE_STATUS = {
  queried: 'bevraagd',
  unreachable: 'onbereikbaar',
  error: 'fout',
  not_queried: 'niet bevraagd',
};

export function sourceStatusText(s) {
  return SOURCE_STATUS[s] ?? String(s ?? '').replace(/_/g, ' ');
}

// What a process is called on screen: the competent authority it acts for
// (RFC-047: one process per actor), otherwise its id.
export function processLabel(p) {
  return p.authority || p.id;
}
