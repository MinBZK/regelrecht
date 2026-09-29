import { describe, expect, it } from 'vitest';
import { emptyFields, portalChannels, rolesOf, sessionText, startScreen } from './channel.js';

// A fictional process with two portal channels and a counter.
const process = {
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

describe('channels and roles', () => {
  it('lists the roles with their id', () => {
    expect(rolesOf(process).map((r) => r.id)).toEqual(['aanvrager', 'burger', 'loket']);
    expect(rolesOf({})).toEqual([]);
  });

  it('picks the start screen from the routes of the role', () => {
    expect(startScreen(process, 'aanvrager')).toBe('possibilities');
    expect(startScreen(process, 'loket')).toBe('counter');
    expect(startScreen(process, 'onbekend')).toBeNull();
  });

  it('gives the channels of the portal, each once', () => {
    expect(portalChannels(process).map((c) => [c.id, c.role])).toEqual([
      ['org', 'Aanvrager'],
      ['burger', 'burger'],
    ]);
  });

  it('writes out the session in the order of the fields', () => {
    const s = { role: 'aanvrager', channel: 'org', fields: { persoon: 'A. Tester', nummer: '12345678' } };
    expect(sessionText(process, s)).toBe('12345678, A. Tester, Aanvrager');
    expect(sessionText(process, null)).toBe('');
  });

  it('fills the fields empty or from an example', () => {
    expect(emptyFields(process.channels.org)).toEqual({ nummer: '', persoon: '' });
    expect(emptyFields(process.channels.org, { nummer: '1' })).toEqual({ nummer: '1', persoon: '' });
  });
});
