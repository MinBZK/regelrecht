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
  if (abs >= 1e9) return `€ ${(euros / 1e9).toLocaleString('nl-NL', { maximumFractionDigits: 1 })} mld`;
  if (abs >= 1e6) return `€ ${(euros / 1e6).toLocaleString('nl-NL', { maximumFractionDigits: 1 })} mln`;
  if (abs >= 1e3) return `€ ${(euros / 1e3).toLocaleString('nl-NL', { maximumFractionDigits: 0 })} dzd`;
  return euroWhole(cents);
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

/** Nette leesbare naam voor een regime-code. */
const REGIME_LABELS = {
  SF15_OUD: 'SF15-oud',
  SF15_NIEUW: 'SF15-nieuw',
  SF15_LLLK: 'SF15-LLLK',
  SF35: 'SF35',
};

export function regimeLabel(regime) {
  return REGIME_LABELS[regime] ?? regime ?? '—';
}

/** Kleur (rijkskleur-token) per regime, voor tags en grafieken. */
const REGIME_COLORS = {
  SF15_OUD: 'oranje',
  SF15_NIEUW: 'lintblauw',
  SF15_LLLK: 'paars',
  SF35: 'groen',
};

export function regimeColor(regime) {
  return REGIME_COLORS[regime] ?? 'neutral';
}
