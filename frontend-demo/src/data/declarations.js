/**
 * Wat een persona al eerder aan de overheid opgaf, als antwoord op de vragen
 * van een wet.
 *
 * Een accijnsaangifte is een eigen opgave: Café Noon geeft per periode op
 * hoeveel bier de brouwerij verliet en hoe sterk het was. De wet vraagt dat
 * als parameters, net als de oppervlakte van een terras. Staat de aangifte in
 * het profiel (`declarations` in demo-config.yaml, per wet), dan rekent de
 * tegel er meteen mee in plaats van ernaar te vragen. Ze telt als een
 * goedgekeurde eigen opgave ("door jou opgegeven"); een correctie van de
 * presentator gaat erboven.
 *
 * @param {{declarations?: Record<string, Record<string, unknown>>}|null} profile
 * @param {string} lawId
 * @param {string} input
 */
export function declaredClaim(profile, lawId, input) {
  const answers = profile?.declarations?.[lawId];
  if (!answers || !(input in answers)) return null;
  return {
    id: `declared:${lawId}:${input}`,
    lawId,
    input,
    newValue: answers[input],
    status: 'APPROVED',
    selfDeclared: true,
    declared: true,
  };
}
