import { describe, it, expect } from 'vitest';
import { tableToRecords } from './actions.js';

// De opslag- en dispatchkant van een `Given the following "..." data with key
// "..."`-stap. De Rust-referentie (`rows_to_records`, packages/engine/tests/
// bdd/dispatch.rs:438) leest elke cel via `headers.get(i)`; deze functie is
// daar de spiegel van, dus dezelfde regel geldt: een kolom hoort bij zijn
// kopnaam, niet bij zijn plaats in de rij.
describe('tableToRecords', () => {
  // Letterlijk de "insurance"-tabel uit corpus/regulation/nl/wet/
  // wet_op_de_zorgtoeslag/scenarios/eligibility.feature - een tabel zoals de
  // engine hem echt krijgt, met drie verschillende celtypen naast elkaar.
  const insurance = [
    ['bsn', 'polis_status', 'verdragsinschrijving'],
    ['999993653', 'ACTIEF', 'false'],
  ];

  it('bindt elke waarde aan haar kopnaam en typeert hem', () => {
    expect(tableToRecords(insurance)).toEqual([
      { bsn: 999993653, polis_status: 'ACTIEF', verdragsinschrijving: false },
    ]);
  });

  it('leest op kopnaam, niet op plaats in de rij', () => {
    // Dezelfde gegevens, kolommen omgewisseld. Wie op positie zou lezen levert
    // hier een ander record op; op naam is het uitkomst-identiek.
    const herschikt = [
      ['verdragsinschrijving', 'bsn', 'polis_status'],
      ['false', '999993653', 'ACTIEF'],
    ];
    expect(tableToRecords(herschikt)).toEqual(tableToRecords(insurance));
  });

  it('geeft niets terug voor een tabel zonder rijen onder de kop', () => {
    expect(tableToRecords([['bsn', 'polis_status']])).toEqual([]);
    expect(tableToRecords(null)).toEqual([]);
  });
});
