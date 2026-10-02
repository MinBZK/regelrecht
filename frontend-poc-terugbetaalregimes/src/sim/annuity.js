/**
 * Annuïteitsrekenwerk voor het wettelijk maandbedrag (WSF 2000 art. 6.9-sfeer).
 *
 * De engine kent geen POWER-operatie, dus de discontofactor (1+r)^-n wordt
 * hier berekend en als parameter aan de wet meegegeven. Zie de TODO in
 * corpus/regulation/nl/wet/wet_studiefinanciering_2000/.
 */

/** Maandrente uit een jaarlijks rentepercentage (ratio, bv. 0.0221). */
export function monthlyRate(annualRate) {
  return annualRate / 12;
}

/** Discontofactor (1+r)^-n voor maandrente r en n resterende maanden. */
export function discountFactor(annualRate, months) {
  const r = monthlyRate(annualRate);
  if (r === 0) return 1;
  return Math.pow(1 + r, -months);
}

/**
 * Annuïtair maandbedrag in eurocent (afgerond op hele centen).
 *
 * @param {number} debtCents - restschuld in eurocent
 * @param {number} annualRate - jaarrente als ratio
 * @param {number} months - resterende terugbetaalmaanden
 */
export function annuityPayment(debtCents, annualRate, months) {
  if (months <= 0) return debtCents;
  const r = monthlyRate(annualRate);
  if (r === 0) return Math.round(debtCents / months);
  const factor = discountFactor(annualRate, months);
  return Math.round((debtCents * r) / (1 - factor));
}
