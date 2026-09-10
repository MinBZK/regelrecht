/**
 * Wat machtigen doet met de rest van de demo: over wie de wet rekent, welke
 * regelingen er staan, en wat er wel en niet mag. Dit is de integratie tussen
 * `data/delegation.js` en de rest van de store, dus die logica staat hier los
 * getest — de store zelf trekt een corpus en een engine binnen, en die zijn er
 * in een unittest niet.
 */
import { describe, expect, it } from 'vitest';
import { computed, ref } from 'vue';
import { delegationKey, delegationsFor, maySubmitClaims } from '../data/delegation.js';

/**
 * Dezelfde afleidingen als in demoStore.js, met de invoer als losse refs. Wat
 * hier bewezen wordt is de regel, niet de bedrading.
 */
function makeContext({ delegations = [], profile, delegationEnabledFlag = true }) {
  const key = ref(null);
  const list = computed(() => (delegationEnabledFlag ? delegations : []));
  const active = computed(() => {
    if (!key.value) return null;
    const found = list.value.find((d) => delegationKey(d) === key.value) ?? null;
    return found && found.subjectType !== 'SELF' ? found : null;
  });
  const setDelegation = (d) => {
    key.value = !d || d.subjectType === 'SELF' ? null : delegationKey(d);
  };
  const personaParams = () => {
    const d = active.value;
    if (d?.subjectType === 'BUSINESS') return { kvk_nummer: d.subjectId };
    if (d?.subjectType === 'CITIZEN') return { bsn: d.subjectId };
    const params = { bsn: profile.bsn };
    if (profile.kvk) params.kvk_nummer = profile.kvk;
    return params;
  };
  const subjectBsn = () => (active.value?.subjectType === 'CITIZEN' ? active.value.subjectId : profile.bsn);
  const wantedDiscoverable = computed(() => {
    const d = active.value;
    if (d) return d.subjectType === 'BUSINESS' ? 'BUSINESS' : 'CITIZEN';
    return profile.type === 'ondernemer' ? 'BUSINESS' : 'CITIZEN';
  });
  return {
    list,
    active,
    setDelegation,
    personaParams,
    subjectBsn,
    wantedDiscoverable,
    canSubmitClaims: computed(() => maySubmitClaims(active.value)),
  };
}

const MERIJN = { bsn: '999100001', kvk: null, type: 'burger' };

const KIND = {
  subjectId: '999200001',
  subjectName: 'Kind 1 van der Meer',
  subjectType: 'CITIZEN',
  delegationType: 'OUDERLIJK_GEZAG',
  permissions: ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'],
  validFrom: null,
  validUntil: null,
  lawId: 'burgerlijk_wetboek_gezag',
  lawName: 'Gezag',
};

const EIGEN_ZAAK = {
  subjectId: '12345678',
  subjectName: 'Thuiszorg Van der Meer',
  subjectType: 'BUSINESS',
  delegationType: 'EIGENAAR',
  permissions: ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'],
  validFrom: null,
  validUntil: null,
  lawId: 'machtigingenwet',
  lawName: 'Machtigingenwet',
};

/** Vennoot met alleen leesrecht: de machtigingenwet geeft hier minder. */
const VENNOOTSCHAP = {
  ...EIGEN_ZAAK,
  subjectId: '99001122',
  subjectName: 'Zorggroep Nederland VOF',
  delegationType: 'VENNOOT',
  permissions: ['LEZEN'],
};

const ZELF = {
  subjectId: MERIJN.bsn,
  subjectName: 'Mezelf',
  subjectType: 'SELF',
  delegationType: 'EIGEN_ZAKEN',
  permissions: ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'],
  validFrom: null,
  validUntil: null,
  lawId: null,
  lawName: null,
};

describe('handelen namens een ander', () => {
  const ctx = () => makeContext({ delegations: [ZELF, KIND, EIGEN_ZAAK, VENNOOTSCHAP], profile: MERIJN });

  it('rekent standaard over de ingelogde burger zelf', () => {
    const c = ctx();
    expect(c.active.value).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999100001' });
    expect(c.subjectBsn()).toBe('999100001');
  });

  it('bevraagt de wet op het kind zodra er namens het kind gehandeld wordt', () => {
    const c = ctx();
    c.setDelegation(KIND);
    expect(c.personaParams()).toEqual({ bsn: '999200001' });
    expect(c.subjectBsn()).toBe('999200001');
  });

  it('bevraagt de wet op het KvK-nummer namens een onderneming', () => {
    const c = ctx();
    c.setDelegation(EIGEN_ZAAK);
    expect(c.personaParams()).toEqual({ kvk_nummer: '12345678' });
    // Een onderneming heeft geen BSN; correcties blijven bij de gemachtigde.
    expect(c.subjectBsn()).toBe('999100001');
  });

  it('toont ondernemersregelingen namens een onderneming en burgerregelingen namens een kind', () => {
    const c = ctx();
    expect(c.wantedDiscoverable.value).toBe('CITIZEN');
    c.setDelegation(EIGEN_ZAAK);
    expect(c.wantedDiscoverable.value).toBe('BUSINESS');
    c.setDelegation(KIND);
    expect(c.wantedDiscoverable.value).toBe('CITIZEN');
  });

  it('behandelt "Mezelf" als het ontbreken van een machtiging', () => {
    const c = ctx();
    c.setDelegation(KIND);
    expect(c.active.value).not.toBeNull();
    c.setDelegation(ZELF);
    expect(c.active.value).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999100001' });
  });

  it('staat corrigeren toe met het recht daartoe en verbiedt het zonder', () => {
    const c = ctx();
    expect(c.canSubmitClaims.value).toBe(true);
    c.setDelegation(EIGEN_ZAAK);
    expect(c.canSubmitClaims.value).toBe(true);
    c.setDelegation(VENNOOTSCHAP);
    expect(c.canSubmitClaims.value).toBe(false);
  });

  it('valt terug op de burger zelf als de bewaarde machtiging niet meer bestaat', () => {
    // Een keuze uit een vorige sessie die de wet nu niet meer geeft: bijvoorbeeld
    // een kind dat meerderjarig is geworden.
    const c = makeContext({ delegations: [ZELF], profile: MERIJN });
    c.setDelegation(KIND);
    expect(c.active.value).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999100001' });
    expect(c.canSubmitClaims.value).toBe(true);
  });

  it('geeft geen machtigingen als de vlag voor dit profiel uit staat', () => {
    const c = makeContext({ delegations: [ZELF, KIND], profile: MERIJN, delegationEnabledFlag: false });
    expect(c.list.value).toEqual([]);
    c.setDelegation(KIND);
    expect(c.active.value).toBeNull();
  });
});

describe('delegationsFor tegen de echte interface', () => {
  it('leest de parallelle lijsten zoals de wetten ze opleveren', () => {
    const law = {
      id: 'burgerlijk_wetboek_gezag',
      name: 'Gezag',
      service: 'RvIG',
      doc: {
        discoverable: 'DELEGATION_PROVIDER',
        articles: [
          {
            machine_readable: {
              execution: {
                output: [
                  'heeft_delegaties',
                  'subject_ids',
                  'subject_names',
                  'subject_types',
                  'delegation_types',
                  'permissions',
                  'valid_from_dates',
                  'valid_until_dates',
                ].map((name) => ({ name })),
              },
            },
          },
        ],
      },
    };
    const engine = {
      executeMultipleWithTrace: () => ({
        outputs: {
          heeft_delegaties: true,
          subject_ids: ['999200001'],
          subject_names: ['Kind 1 van der Meer'],
          subject_types: ['CITIZEN'],
          delegation_types: ['OUDERLIJK_GEZAG'],
          permissions: [['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN']],
          valid_from_dates: ['2020-01-01'],
          valid_until_dates: [null],
        },
      }),
    };
    const corpus = { latestById: new Map([[law.id, law]]) };
    const { delegations } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    expect(delegations).toHaveLength(1);
    expect(delegations[0]).toMatchObject({
      subjectId: '999200001',
      subjectType: 'CITIZEN',
      delegationType: 'OUDERLIJK_GEZAG',
      lawName: 'Gezag',
    });
  });
});
