/**
 * Welke correcties bij welke zaak horen.
 *
 * Twee vragen die op elkaar lijken maar niet hetzelfde zijn, en die in
 * `resubmitCase` één keer door elkaar liepen:
 *
 * - *Moet er een mens naar kijken?* Daarvoor telt élke openstaande correctie
 *   van deze persoon mee, ook een die bij een andere regeling is opgegeven. Een
 *   inkomenswijziging bij de huurtoeslag verandert immers ook de zorgtoeslag.
 * - *Bij welke zaak hoort deze correctie?* Daarvoor telt alleen de regeling
 *   waarop de correctie is ingediend.
 *
 * Die tweede vraag met het antwoord van de eerste beantwoorden, hangt een
 * correctie voorgoed aan de verkeerde zaak: het veld wordt maar één keer gezet,
 * dus de zaak waar hij wél bij hoort krijgt hem daarna niet meer.
 */
import { describe, expect, it } from 'vitest';

/** De regel uit `demoStore.resubmitCase`, los van de store getest. */
function assignOwnership(claims, caseRecord) {
  for (const claim of claims) {
    if (!claim.caseId && claim.tileLawId === caseRecord.lawId) claim.caseId = caseRecord.id;
  }
  return claims;
}

const claim = (over) => ({ id: 'c1', caseId: null, tileLawId: 'huurtoeslag', ...over });

describe('eigenaarschap van een correctie', () => {
  it('hangt een correctie aan de zaak van dezelfde regeling', () => {
    const claims = [claim({ id: 'c1', tileLawId: 'huurtoeslag' })];
    assignOwnership(claims, { id: 'zaak-1', lawId: 'huurtoeslag' });
    expect(claims[0].caseId).toBe('zaak-1');
  });

  it('laat een correctie van een andere regeling met rust', () => {
    // Deze correctie telt wél mee voor de vraag of er beoordeeld moet worden,
    // maar hoort in het dossier van de zorgtoeslag en niet hier.
    const claims = [claim({ id: 'c2', tileLawId: 'zorgtoeslagwet' })];
    assignOwnership(claims, { id: 'zaak-1', lawId: 'huurtoeslag' });
    expect(claims[0].caseId).toBeNull();
  });

  it('laat een correctie die al bij een zaak hoort staan waar hij staat', () => {
    const claims = [claim({ id: 'c3', caseId: 'zaak-eerder' })];
    assignOwnership(claims, { id: 'zaak-1', lawId: 'huurtoeslag' });
    expect(claims[0].caseId).toBe('zaak-eerder');
  });

  it('houdt twee zaken uit elkaar', () => {
    // Het geval waar het misging: één persoon, twee lopende regelingen. Wie het
    // eerst opnieuw indient, mocht niet de correcties van de ander opeisen.
    const claims = [
      claim({ id: 'huur', tileLawId: 'huurtoeslag' }),
      claim({ id: 'zorg', tileLawId: 'zorgtoeslagwet' }),
    ];
    assignOwnership(claims, { id: 'zaak-huur', lawId: 'huurtoeslag' });
    assignOwnership(claims, { id: 'zaak-zorg', lawId: 'zorgtoeslagwet' });
    expect(claims.map((c) => c.caseId)).toEqual(['zaak-huur', 'zaak-zorg']);
  });
});
