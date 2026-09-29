import { describe, expect, it } from 'vitest';
import { beginscherm, lege, portaalkanalen, rollenVan, sessieTekst } from './kanaal.js';

// Een fictief proces met twee portaalkanalen en een loket.
const proces = {
  portal: true,
  handling: null,
  counter: true,
  channels: {
    org: { label: 'Organisatie', fields: [{ name: 'nummer', label: 'Nummer' }, { name: 'persoon', label: 'Naam' }] },
    burger: { label: 'Burger', fields: [{ name: 'nummer', label: 'Burgernummer' }] },
    medewerker: { label: 'Medewerker', fields: [{ name: 'naam', label: 'Naam' }] },
  },
  roles: {
    aanvrager: { channel: 'org', routes: ['portal'], label: 'Aanvrager' },
    burger: { channel: 'burger', routes: ['portal'], label: 'burger' },
    loket: { channel: 'medewerker', routes: ['counter'], label: 'Loket' },
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
    expect(beginscherm(proces, 'onbekend')).toBeNull();
  });

  it('geeft de kanalen van het portaal, elk een keer', () => {
    expect(portaalkanalen(proces).map((k) => [k.id, k.rol])).toEqual([
      ['org', 'Aanvrager'],
      ['burger', 'burger'],
    ]);
  });

  it('schrijft de sessie uit in de volgorde van de velden', () => {
    const s = { role: 'aanvrager', channel: 'org', fields: { persoon: 'A. Tester', nummer: '12345678' } };
    expect(sessieTekst(proces, s)).toBe('12345678, A. Tester, Aanvrager');
    expect(sessieTekst(proces, null)).toBe('');
  });

  it('vult de velden leeg of uit een voorbeeld', () => {
    expect(lege(proces.channels.org)).toEqual({ nummer: '', persoon: '' });
    expect(lege(proces.channels.org, { nummer: '1' })).toEqual({ nummer: '1', persoon: '' });
  });
});
