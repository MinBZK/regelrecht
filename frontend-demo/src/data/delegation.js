/**
 * Wie mag namens wie handelen.
 *
 * Machtiging staat niet in de app maar in de wet: elke wet die het
 * machtigingscontract vervult levert de delegaties die zij regelt.
 * Het gezag over een minderjarige komt uit het Burgerlijk Wetboek, het
 * vertegenwoordigen van een onderneming uit de Machtigingenwet, bewind en
 * curatele uit hun eigen wetten. De demo evalueert die wetten met dezelfde
 * engine als de rest van het portaal en zet de uitkomst om in een lijst.
 *
 * Elke provider-wet levert dezelfde uitvoer, als parallelle lijsten (de
 * standaardinterface uit de POC, `machine/delegation/manager.py`):
 *
 *   heeft_delegaties   boolean
 *   subject_ids        ['999200001', ...]
 *   subject_names      ['Kind 1 van der Meer', ...]
 *   subject_types      ['CITIZEN' | 'BUSINESS' | 'SELF', ...]
 *   delegation_types   ['OUDERLIJK_GEZAG', 'EIGENAAR', ...]
 *   permissions        [['LEZEN', 'CLAIMS_INDIENEN'], ...]
 *   valid_from_dates   ['2020-01-01', ...]
 *   valid_until_dates  ['2038-01-01' | null, ...]
 *
 * Een wet die niets oplevert zegt `heeft_delegaties: false`; een wet die de
 * engine niet kan uitvoeren telt niet mee en blokkeert de rest niet.
 */

import { isDelegationProvider } from './entrypoints.js';

/** De uitvoernamen van de standaardinterface, in de volgorde van de lijsten. */
const OUTPUTS = [
  'heeft_delegaties',
  'subject_ids',
  'subject_names',
  'subject_types',
  'delegation_types',
  'permissions',
  'valid_from_dates',
  'valid_until_dates',
];

/**
 * De rechten, van licht naar zwaar. Bepaalt de volgorde waarin ze getoond
 * worden en welk recht overblijft als twee wetten hetzelfde onderwerp
 * verschillend regelen.
 */
export const PERMISSION_ORDER = ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'];

/** Wat een recht in het Nederlands betekent, voor de uitleg in het scherm. */
export const PERMISSION_LABELS = {
  LEZEN: 'Gegevens inzien',
  CLAIMS_INDIENEN: 'Gegevens corrigeren en aanvragen indienen',
  BESLUITEN_ONTVANGEN: 'Besluiten ontvangen',
};

/** Wat een soort machtiging in het Nederlands heet. */
export const DELEGATION_TYPE_LABELS = {
  EIGEN_ZAKEN: 'Eigen zaken',
  OUDERLIJK_GEZAG: 'Ouderlijk gezag',
  GEZAMENLIJK_GEZAG: 'Gezamenlijk gezag',
  VOOGDIJ: 'Voogdij',
  VOOGDIJ_INSTELLING: 'Voogdij (instelling)',
  EIGENAAR: 'Eigenaar',
  VENNOOT: 'Vennoot',
  BESTUURDER: 'Bestuurder',
  GEVOLMACHTIGDE: 'Gevolmachtigde',
  CURATOR: 'Curator',
  BEWINDVOERDER: 'Bewindvoerder',
  MENTOR: 'Mentor',
  EXECUTEUR: 'Executeur',
};

/** De wetten die machtigingen leveren, in een vaste volgorde. */
export function delegationProviders(corpus) {
  if (!corpus) return [];
  return [...corpus.latestById.values()]
    .filter((law) => isDelegationProvider(law.doc))
    .sort((a, b) => a.id.localeCompare(b.id));
}

/** De uitvoernamen die deze wet werkelijk declareert. */
function declaredOutputs(law) {
  const names = new Set(
    (law.doc?.articles ?? [])
      .flatMap((a) => a.machine_readable?.execution?.output ?? [])
      .map((o) => o.name),
  );
  return OUTPUTS.filter((name) => names.has(name));
}

/** Een lijstuitvoer als array, ook als de engine één waarde teruggaf. */
function asList(value) {
  if (value === null || value === undefined) return [];
  return Array.isArray(value) ? value : [value];
}

/**
 * De delegaties die één wet voor deze persoon oplevert.
 *
 * @returns {{delegations: Array, error: string|null}}
 */
function delegationsFromLaw(law, outputs) {
  if (!outputs.heeft_delegaties) return [];
  const ids = asList(outputs.subject_ids);
  const names = asList(outputs.subject_names);
  const types = asList(outputs.subject_types);
  const kinds = asList(outputs.delegation_types);
  const permissions = asList(outputs.permissions);
  const from = asList(outputs.valid_from_dates);
  const until = asList(outputs.valid_until_dates);

  return ids.map((id, i) => ({
    subjectId: String(id),
    subjectName: names[i] ?? String(id),
    // Zonder opgave is het een burger: dat is wat de meeste wetten regelen.
    subjectType: types[i] ?? 'CITIZEN',
    delegationType: kinds[i] ?? null,
    permissions: asList(permissions[i]),
    validFrom: from[i] ?? null,
    validUntil: until[i] ?? null,
    // Waar deze machtiging vandaan komt: de wet is de verantwoording.
    lawId: law.id,
    lawName: law.name,
    service: law.service,
  }));
}

/**
 * Een machtiging geldt alleen binnen haar termijn. Een wet die zelf al op de
 * peildatum rekent levert niets buiten de termijn, maar niet elke wet doet
 * dat, en een demo die een verlopen machtiging toont is misleidend.
 *
 * Dit werkt door in wat er te zien is, en dat is de bedoeling: Claudia's
 * koffiezaak staat sinds 2025-01-15 in het handelsregister, dus op een
 * peildatum daarvóór heeft zij die machtiging nog niet en verdwijnt de keuze
 * uit de werkbalk. Wie zich afvraagt waarom de knop weg is bij een vroege
 * peildatum: dat is de wet, niet een fout.
 */
function isValidOn(delegation, referenceDate) {
  if (!referenceDate) return true;
  if (delegation.validFrom && String(delegation.validFrom) > referenceDate) return false;
  if (delegation.validUntil && String(delegation.validUntil) < referenceDate) return false;
  return true;
}

/** De rechten in vaste volgorde, zonder dubbelen en zonder onbekende. */
function orderPermissions(permissions) {
  const held = new Set(permissions.map(String));
  return PERMISSION_ORDER.filter((p) => held.has(p));
}

/**
 * Alle machtigingen van één persoon, over alle provider-wetten heen.
 *
 * `SELF` (handelen namens jezelf) wordt door meerdere wetten geleverd:
 * minderjarigheid en handelingsonbekwaamheid zeggen er allebei iets over. Ze
 * worden samengevoegd tot één, met de doorsnede van de rechten: de wet die het
 * meest beperkt wint, want een meerderjarige die onder curatele staat mag niet
 * meer omdat een andere wet zwijgt. Blijft er niets over, dan resteert LEZEN.
 *
 * @param {object} engine        de WASM-engine
 * @param {object} corpus        het geladen corpus
 * @param {string} bsn           degene die handelt
 * @param {string} referenceDate peildatum
 * @returns {{delegations: Array, errors: Array<{lawId: string, message: string}>}}
 */
export function delegationsFor(engine, corpus, bsn, referenceDate) {
  const delegations = [];
  const errors = [];
  if (!engine || !corpus || !bsn) return { delegations, errors };

  for (const law of delegationProviders(corpus)) {
    const want = declaredOutputs(law);
    // Zonder `heeft_delegaties` spreekt de wet de interface niet; overslaan.
    if (!want.includes('heeft_delegaties')) continue;
    try {
      const result = engine.executeMultipleWithTrace(law.id, want, { bsn }, referenceDate);
      delegations.push(...delegationsFromLaw(law, result?.outputs ?? {}));
    } catch (e) {
      const message = typeof e === 'string' ? e : e?.error ?? e?.message ?? JSON.stringify(e);
      errors.push({ lawId: law.id, message: String(message) });
    }
  }

  const valid = delegations.filter((d) => isValidOn(d, referenceDate));
  const selves = valid.filter((d) => d.subjectType === 'SELF');
  const others = valid.filter((d) => d.subjectType !== 'SELF');

  const merged = [];
  if (selves.length) {
    let permissions = new Set(orderPermissions(selves[0].permissions));
    for (const d of selves.slice(1)) {
      const own = new Set(orderPermissions(d.permissions));
      permissions = new Set([...permissions].filter((p) => own.has(p)));
    }
    merged.push({
      ...selves[0],
      subjectId: bsn,
      subjectName: 'Mezelf',
      subjectType: 'SELF',
      delegationType: 'EIGEN_ZAKEN',
      permissions: permissions.size ? PERMISSION_ORDER.filter((p) => permissions.has(p)) : ['LEZEN'],
      validFrom: null,
      validUntil: null,
      // Samengevoegd uit meerdere wetten: noem ze allemaal.
      lawId: selves.length === 1 ? selves[0].lawId : null,
      lawName: selves.length === 1 ? selves[0].lawName : null,
      sourceLaws: selves.map((d) => ({ id: d.lawId, name: d.lawName })),
    });
  }

  for (const d of others) merged.push({ ...d, permissions: orderPermissions(d.permissions) });

  // Mezelf bovenaan, daarna op naam: de eigen zaken zijn het vertrekpunt.
  merged.sort((a, b) => {
    if ((a.subjectType === 'SELF') !== (b.subjectType === 'SELF')) return a.subjectType === 'SELF' ? -1 : 1;
    return String(a.subjectName).localeCompare(String(b.subjectName));
  });

  return { delegations: merged, errors };
}

/**
 * Hoe de machtiging heet in het scherm: 'Ouderlijk gezag', 'Eigenaar'. Een
 * soort die geen label heeft valt terug op de code uit de wet, want die is
 * altijd beter dan niets.
 */
export function delegationLabel(delegation) {
  if (!delegation) return null;
  const type = delegation.delegationType;
  return DELEGATION_TYPE_LABELS[type] ?? type ?? null;
}

/** Mag er met deze machtiging gecorrigeerd en aangevraagd worden? */
export function maySubmitClaims(delegation) {
  if (!delegation) return true;
  return delegation.permissions.includes('CLAIMS_INDIENEN');
}

/** De sleutel waarmee een machtiging in de opgeslagen staat wordt aangeduid. */
export function delegationKey(delegation) {
  return delegation ? `${delegation.subjectType}:${delegation.subjectId}` : null;
}
