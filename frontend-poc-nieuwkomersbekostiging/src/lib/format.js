/**
 * Weergave-helpers: alle geldbedragen in de simulatie zijn integer eurocent,
 * alle percentages zijn ratio's (0.12 = 12%). Deze module vertaalt ze naar
 * nl-NL-tekst voor de UI.
 */

const euroFormatter = new Intl.NumberFormat('nl-NL', {
  style: 'currency',
  currency: 'EUR',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

const euroWholeFormatter = new Intl.NumberFormat('nl-NL', {
  style: 'currency',
  currency: 'EUR',
  minimumFractionDigits: 0,
  maximumFractionDigits: 0,
});

/** Eurocent → "€ 1.234,56". */
export function euro(cents) {
  if (cents === null || cents === undefined || Number.isNaN(cents)) return '—';
  return euroFormatter.format(cents / 100);
}

/** Eurocent → "€ 1.235" (afgerond op hele euro's, voor grote totalen). */
export function euroWhole(cents) {
  if (cents === null || cents === undefined || Number.isNaN(cents)) return '—';
  return euroWholeFormatter.format(Math.round(cents / 100));
}

/** Eurocent → compacte "€ 1,2 mln" / "€ 340 dzd" voor populatie-totalen. */
export function euroCompact(cents) {
  if (cents === null || cents === undefined || Number.isNaN(cents)) return '—';
  const euros = cents / 100;
  const abs = Math.abs(euros);
  if (abs >= 1e9) return `€ ${(euros / 1e9).toLocaleString('nl-NL', { maximumFractionDigits: 2 })} mld`;
  if (abs >= 1e6) return `€ ${(euros / 1e6).toLocaleString('nl-NL', { maximumFractionDigits: 1 })} mln`;
  if (abs >= 1e3) return `€ ${(euros / 1e3).toLocaleString('nl-NL', { maximumFractionDigits: 0 })} dzd`;
  return euroWhole(cents);
}

/** Verschil in eurocent met teken: "+ € 1,2 mln" / "− € 340 dzd" / "±0". */
export function euroDelta(cents) {
  if (cents === null || cents === undefined || Number.isNaN(cents)) return '—';
  if (Math.abs(cents) < 50) return '± 0';
  return `${cents > 0 ? '+' : '−'} ${euroCompact(Math.abs(cents))}`;
}

/** Ratio → "12%" (0 decimalen) of "12,3%" (met decimalen). */
export function percent(ratio, decimals = 0) {
  if (ratio === null || ratio === undefined || Number.isNaN(ratio)) return '—';
  return `${(ratio * 100).toLocaleString('nl-NL', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })}%`;
}

/** Geheel getal met duizendtalscheiding: 1234567 → "1.234.567". */
export function number(value) {
  if (value === null || value === undefined || Number.isNaN(value)) return '—';
  return Math.round(value).toLocaleString('nl-NL');
}

/** Getal met decimalen: 1234.5 → "1.234,5". */
export function decimal(value, decimals = 1) {
  if (value === null || value === undefined || Number.isNaN(value)) return '—';
  return value.toLocaleString('nl-NL', { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
}

/** Aantal met teken voor verschillen. */
export function numberDelta(value) {
  if (value === null || value === undefined || Number.isNaN(value)) return '—';
  if (Math.abs(value) < 0.5) return '± 0';
  return `${value > 0 ? '+' : '−'} ${number(Math.abs(value))}`;
}

/** Uren → "1.234 uur" (of "12,5 uur" onder de honderd). */
export function hours(uren) {
  if (uren === null || uren === undefined || Number.isNaN(uren)) return '—';
  return uren >= 100 ? `${number(uren)} uur` : `${decimal(uren, 1)} uur`;
}

/** Minuten → "45 min" / "2 uur 15 min". */
export function minutes(min) {
  if (min === null || min === undefined || Number.isNaN(min)) return '—';
  const m = Math.round(min);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  const rest = m % 60;
  return rest ? `${h} uur ${rest} min` : `${h} uur`;
}

/** Jaren als "2,3 jaar"; null → "nooit". */
export function years(value) {
  if (value === null || value === undefined || Number.isNaN(value)) return 'nooit';
  if (value === Infinity) return 'nooit';
  if (value === 0) return 'geen investering';
  return `${decimal(value, 1)} jaar`;
}

/** Maanden → leesbare duur "12 jaar en 4 maanden". */
export function duration(maanden) {
  if (maanden === null || maanden === undefined) return '—';
  const jaren = Math.floor(maanden / 12);
  const rest = maanden % 12;
  const delen = [];
  if (jaren > 0) delen.push(`${jaren} jaar`);
  if (rest > 0) delen.push(`${rest} ${rest === 1 ? 'maand' : 'maanden'}`);
  return delen.length ? delen.join(' en ') : '0 maanden';
}

/** Toon een waarde met de juiste eenheid op basis van een definitie-unit. */
export function byUnit(value, unit) {
  if (unit === 'eurocent') return euro(value);
  if (unit === 'ratio') return percent(value, value < 0.1 ? 1 : 0);
  return number(value);
}

/** Boolean → "ja"/"nee", null → "—". */
export function jaNee(value) {
  if (value === null || value === undefined) return '—';
  return value ? 'ja' : 'nee';
}
