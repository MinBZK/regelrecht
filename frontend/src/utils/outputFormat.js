/**
 * Shared output formatting and comparison utilities used by
 * ScenarioForm and ExecutionTraceView.
 */

export function formatValue(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  return String(value);
}

export function formatOutputValue(value, name) {
  const raw = formatValue(value);
  if (typeof value === 'number' && Number.isInteger(value) &&
      (name.includes('hoogte') || name.includes('bedrag') || name.includes('premie'))) {
    return `${raw} (${(value / 100).toFixed(2)} euro)`;
  }
  return raw;
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
