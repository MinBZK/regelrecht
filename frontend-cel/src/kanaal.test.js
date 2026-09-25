import { describe, expect, it } from 'vitest';
import { beginscherm, lege, portaalkanalen, rollenVan, sessieTekst } from './kanaal.js';

// Een fictief proces met twee portaalkanalen en een loket.
const proces = {
  portaal: true,
  behandeling: null,
  loket: true,
  kanalen: {
    org: { label: 'Organisatie', velden: [{ naam: 'nummer', label: 'Nummer' }, { naam: 'persoon', label: 'Naam' }] },
    burger: { label: 'Burger', velden: [{ naam: 'nummer', label: 'Burgernummer' }] },
    medewerker: { label: 'Medewerker', velden: [{ naam: 'naam', label: 'Naam' }] },
  },
  rollen: {
    aanvrager: { kanaal: 'org', routes: ['portaal'], label: 'Aanvrager' },
    burger: { kanaal: 'burger', routes: ['portaal'], label: 'burger' },
    loket: { kanaal: 'medewerker', routes: ['loket'], label: 'Loket' },
  },
};

describe('kanalen en rollen', () => {
  it('somt de rollen op met hun id', () => {
    expect(rollenVan(proces).map((r) => r.id)).toEqual(['aanvrager', 'burger', 'loket']);
    expect(rollenVan({})).toEqual([]);
  });

  it('kiest het beginscherm uit de routes van de rol', () => {
    expect(beginscherm(proces, 'aanvrager')).toBe('mogelijkheden');
    expect(beginscherm(proces, 'loket')).toBe('loket');
    expect(beginscherm(proces, 'onbekend')).toBe('kroniek');
  });

  it('geeft de kanalen van het portaal, elk een keer', () => {
    expect(portaalkanalen(proces).map((k) => [k.id, k.rol])).toEqual([
      ['org', 'Aanvrager'],
      ['burger', 'burger'],
    ]);
  });

  it('schrijft de sessie uit in de volgorde van de velden', () => {
    const s = { rol: 'aanvrager', kanaal: 'org', velden: { persoon: 'A. Tester', nummer: '12345678' } };
    expect(sessieTekst(proces, s)).toBe('12345678, A. Tester, Aanvrager');
    expect(sessieTekst(proces, null)).toBe('');
  });

  it('vult de velden leeg of uit een voorbeeld', () => {
    expect(lege(proces.kanalen.org)).toEqual({ nummer: '', persoon: '' });
    expect(lege(proces.kanalen.org, { nummer: '1' })).toEqual({ nummer: '1', persoon: '' });
  });
});
