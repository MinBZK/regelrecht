/**
 * Shared output formatting and comparison utilities used by
 * ScenarioForm and ExecutionTraceView.
 */

export function formatValue(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  return String(value);
}

const EURO_FORMATTER = new Intl.NumberFormat('nl-NL', {
  style: 'currency',
  currency: 'EUR',
});

function isMonetary(value, name) {
  return typeof value === 'number' && Number.isInteger(value) &&
    (name.includes('hoogte') || name.includes('bedrag') || name.includes('premie'));
}

export function formatOutputValue(value, name) {
  const raw = formatValue(value);
  if (isMonetary(value, name)) {
    return `${raw} (${EURO_FORMATTER.format(value / 100)})`;
  }
  return raw;
}

/** Returns `{ text, supportingText }` for output rendering. For monetary
 *  integer outputs the euro-formatted value becomes supporting text. */
export function formatOutputValueParts(value, name) {
  const raw = formatValue(value);
  if (isMonetary(value, name)) {
    return { text: raw, supportingText: EURO_FORMATTER.format(value / 100) };
  }
  return { text: raw, supportingText: '' };
}

export function normalizeForCompare(value) {
  if (value === 'true' || value === true) return true;
  if (value === 'false' || value === false) return false;
  if (value === 'null' || value === null) return null;
  if (typeof value === 'string' && /^-?\d+(\.\d+)?$/.test(value)) return Number(value);
  return value;
}

export function matchStatus(outputName, actualValue, expectations) {
  if (!(outputName in expectations)) return 'neutral';
  const expected = expectations[outputName];
  if (expected === null || expected === undefined) return 'neutral';
  const actual = normalizeForCompare(actualValue);
  const exp = normalizeForCompare(expected);
  if (actual === exp) return 'passed';
  if (typeof actual === 'number' && typeof exp === 'number' && Math.abs(actual - exp) < 1e-9) return 'passed';
  return 'failed';
}
