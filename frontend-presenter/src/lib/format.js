/**
 * A value as a slide shows it, in the unit its law declares: eurocent and euro
 * as euros, a ratio as a percentage, the rest through the editor's formatValue
 * (ja/nee, geen, onbekend). Shared by the wet block (literals in the YAML) and
 * the reken block (outputs of the engine).
 */
import { formatValue } from '@editor-utils/outputFormat.js';

const EURO = new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR' });
const PCT = new Intl.NumberFormat('nl-NL', { style: 'percent', maximumFractionDigits: 3 });

export function formatInUnit(value, unit) {
  if (typeof value === 'number') {
    if (unit === 'eurocent') return EURO.format(value / 100);
    if (unit === 'euro') return EURO.format(value);
    if (unit === 'ratio') return PCT.format(value);
  }
  return formatValue(value);
}
