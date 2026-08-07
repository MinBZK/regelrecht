/**
 * Shared output formatting and comparison utilities used by
 * ScenarioForm and ExecutionTraceView.
 */

export function formatValue(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  return String(value);
}

/**
 * Format YAML/engine identifiers for display: underscores → spaces,
 * all-caps strings → lowercase (`BESCHIKKING` → `beschikking`).
 * Mixed-case identifiers (e.g. `Artikel`) are left untouched.
 */
export function humanize(name) {
  if (typeof name !== 'string') return name;
  const spaced = name.replace(/_/g, ' ');
  return /[A-Z]/.test(spaced) && spaced === spaced.toUpperCase() ? spaced.toLowerCase() : spaced;
}

const EURO_FORMATTER = new Intl.NumberFormat('nl-NL', {
  style: 'currency',
  currency: 'EUR',
});

/**
 * Euro rendering of a value, or '' when the field isn't money.
 *
 * The declared `type_spec.unit` is the only signal, matching the engine
 * (`packages/engine/src/units.rs`): a unit labels the value, it never states a
 * conversion the engine performs. `eurocent` stores cents so display divides by
 * 100; `euro` already stores euros. An unannotated or unknown unit gets no
 * currency form at all — guessing from the field name used to hand
 * `toetsingsinkomen` a raw cent count and a name-matched integer a euro sign it
 * had not earned.
 */
function euroText(value, unit) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '';
  if (unit === 'eurocent') return EURO_FORMATTER.format(value / 100);
  if (unit === 'euro') return EURO_FORMATTER.format(value);
  return '';
}

export function formatOutputValue(value, unit) {
  const raw = formatValue(value);
  const euros = euroText(value, unit);
  return euros ? `${raw} (${euros})` : raw;
}

/** Returns `{ text, supportingText }` for output rendering. For monetary
 *  outputs the euro-formatted value becomes supporting text. */
export function formatOutputValueParts(value, unit) {
  return { text: formatValue(value), supportingText: euroText(value, unit) };
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
