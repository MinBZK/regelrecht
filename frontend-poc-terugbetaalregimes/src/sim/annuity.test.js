import { describe, it, expect } from 'vitest';
import { annuityPayment, discountFactor } from './annuity.js';

describe('annuityPayment', () => {
  it('komt overeen met de closed form voor het rekenvoorbeeld', () => {
    // Schuld €25.000, rente 2,21%, 180 maanden (PDF voetnoot 11).
    const bedrag = annuityPayment(2500000, 0.0221, 180);
    const r = 0.0221 / 12;
    const verwacht = Math.round((2500000 * r) / (1 - Math.pow(1 + r, -180)));
    expect(bedrag).toBe(verwacht);
    // Orde van grootte: ~€163 per maand.
    expect(bedrag).toBeGreaterThan(16000);
    expect(bedrag).toBeLessThan(17000);
  });

  it('betaalt de schuld exact af in n maanden', () => {
    const schuld = 2500000;
    const rente = 0.0221;
    const n = 180;
    const maandbedrag = annuityPayment(schuld, rente, n);
    let rest = schuld;
    const r = rente / 12;
    for (let m = 0; m < n; m++) {
      rest += Math.round(rest * r);
      rest -= Math.min(maandbedrag, rest);
    }
    // Afrondingsruis van hooguit enkele euro's over 15 jaar.
    expect(Math.abs(rest)).toBeLessThan(1000);
  });

  it('valt terug op lineair aflossen bij rente 0', () => {
    expect(annuityPayment(120000, 0, 12)).toBe(10000);
    expect(discountFactor(0, 180)).toBe(1);
  });
});
