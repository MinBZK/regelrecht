/**
 * Amounts are stored as integer eurocents but displayed/entered as euros.
 * These two pure helpers convert between the two; shared by the editor
 * (EditSheet for law definition values, ScenarioParameterInput for scenario
 * amount inputs) and frontend-cel (a handeling's amount fields and outcomes,
 * when the regulation declares `type_spec.unit: eurocent`).
 */

/** Eurocents (int) -> euros (2-decimal number). Empty/nullish passes through. */
export function centsToEuros(cents) {
  if (cents === '' || cents == null) return '';
  return +(Number(cents) / 100).toFixed(2);
}

/** Euros (number) -> eurocents (rounded int). Empty/nullish passes through. */
export function eurosToCents(euros) {
  if (euros === '' || euros == null) return '';
  return Math.round(Number(euros) * 100);
}
