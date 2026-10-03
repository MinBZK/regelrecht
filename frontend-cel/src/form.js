// From what is in the form to `external` on submission.
import { centsToEuros, eurosToCents } from '@regelrecht/frontend-shared/currency.js';
//
// An empty field is left out: the cell then records it as null, and a
// derivation such as `filled` sees it as not filled in. A name with a dot
// (`a.b`) is nested, like `$external.a.b` in the stream.

export function isEmpty(value) {
  if (value === undefined || value === null) return true;
  if (typeof value === 'string') return value.trim() === '';
  if (Array.isArray(value)) return value.length === 0;
  return false;
}

function cleanRow(row) {
  const out = {};
  for (const [k, v] of Object.entries(row)) {
    if (!isEmpty(v)) out[k] = v;
  }
  return out;
}

// Sets a value under a dotted name, nested.
export function setPath(target, name, value) {
  const parts = name.split('.');
  for (const part of parts.slice(0, -1)) target = target[part] ??= {};
  target[parts.at(-1)] = value;
}

// The value under a dotted name, or null.
export function getPath(source, name) {
  let v = source;
  for (const part of name.split('.')) v = v?.[part];
  return v ?? null;
}

// `fields` (the fields of the form) gives each amount the unit of the law,
// also in a table column; without it the values go as they are.
export function external(values, fields = []) {
  const byName = Object.fromEntries(fields.map((f) => [f.name, f]));
  const out = {};
  for (const [name, value] of Object.entries(values)) {
    let v = value;
    const field = byName[name];
    if (Array.isArray(v)) {
      v = v.map(cleanRow).filter((r) => Object.keys(r).length > 0);
      if (field) v = convertRows(field, v, toLaw);
    } else if (field) {
      v = toLaw(field, v);
    }
    if (isEmpty(v)) continue;
    setPath(out, name, v);
  }
  return out;
}

// The rows of a table field, each cell converted with `convert` per column.
function convertRows(field, rows, convert) {
  const columns = Object.fromEntries((field.columns ?? []).map((c) => [c.id, c]));
  return rows.map((r) =>
    Object.fromEntries(Object.entries(r).map(([k, v]) => [k, columns[k] ? convert(columns[k], v) : v])),
  );
}

// A value of the law (an example, what a register supplies) in the units of
// the form, also per column of a table.
export function formValue(field, v) {
  if (Array.isArray(v)) return convertRows(field, v, toForm);
  return toForm(field, v);
}

// The input an amount gets: a number, in the unit `fieldLabel` names.
export function inputKind(field) {
  return field.type === 'amount' ? 'number' : field.type;
}

// Options from a form are strings or {value, label}.
export function options(list) {
  return (list ?? []).map((o) =>
    typeof o === 'object' && o !== null
      ? { value: o.value, label: o.label ?? String(o.value) }
      : { value: o, label: String(o) },
  );
}

// The text from an input field: nldd fields give it in `detail.value`, a
// plain element in `target.value`.
export function fieldText(e) {
  return e.detail?.value ?? e.target?.value ?? '';
}

// The form asks for an amount in euros when the regulation counts it in
// eurocents or in euros (`unit`, from `type_spec.unit`); the law gets it back
// in its own unit. An amount in another unit, or without one, the form asks
// for as it is.
export function inEuros(field) {
  return field.type === 'amount' && (field.unit === 'eurocent' || field.unit === 'euro');
}

// A filled-in value in the unit of the law.
export function toLaw(field, v) {
  return field.type === 'amount' && field.unit === 'eurocent' && typeof v === 'number' ? eurosToCents(v) : v;
}

// A value of the law in the unit of the form.
export function toForm(field, v) {
  return field.type === 'amount' && field.unit === 'eurocent' && typeof v === 'number' ? centsToEuros(v) : v;
}

// The unit the form asks an amount in, or '' when it names none.
export function formUnit(field) {
  if (inEuros(field)) return 'euro';
  return field.type === 'amount' && field.unit ? field.unit : '';
}

// The label of a field, with the unit the form asks in.
export function fieldLabel(field) {
  const unit = formUnit(field);
  return unit ? `${field.label} (${unit})` : field.label;
}

// What the channel or a register supplies for a field (`supplied` from
// GET /api/form, note "het gram uit de wet"): the portal shows it filled in
// automatically, with where it came from, and does not send it along.
const SOURCE_TEXT = { channel: 'uit het inlogmiddel', register: 'uit het register' };

export function suppliedText(field) {
  const s = field?.supplied;
  if (!s) return '';
  const from = SOURCE_TEXT[s.source] ?? s.source;
  const basis = (s.legal_basis ?? []).join(', ');
  return `Automatisch ingevuld ${from}${basis ? ` (${basis})` : ''}`;
}

// The values without what the channel or a register supplies.
export function withoutSupplied(values, fields) {
  const out = { ...values };
  for (const f of fields ?? []) if (f.supplied) delete out[f.name];
  return out;
}
