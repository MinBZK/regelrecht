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

export function external(values) {
  const out = {};
  for (const [name, value] of Object.entries(values)) {
    let v = value;
    if (Array.isArray(v)) v = v.map(cleanRow).filter((r) => Object.keys(r).length > 0);
    if (isEmpty(v)) continue;
    setPath(out, name, v);
  }
  return out;
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

// The label of a field, with the unit the form asks in.
export function fieldLabel(field) {
  if (inEuros(field)) return `${field.label} (euro)`;
  return field.type === 'amount' && field.unit ? `${field.label} (${field.unit})` : field.label;
}
