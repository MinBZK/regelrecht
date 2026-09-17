/**
 * Een portaal zoals `GET /api/portaal` het geeft, over de wereld van
 * `worldFixture`: de actor, twee fictieve aanvragers en drie vragen.
 *
 * Met de hand geschreven en niet uit de simulator gegenereerd, anders dan het
 * beeld: de vorm staat vast in de API-tests van `packages/chrono-poc-web`, en
 * wat hier telt is dat de pagina's er iets mee kunnen. De waarden volgen de
 * publieke wereld, zodat de formulieren van het beeld erop passen.
 */
import { cloneWorld } from './worldFixture.js';

const vragen = (bsn) => [
  {
    label: 'Beschikking zorgtoeslag',
    cell: 'toeslagen',
    lexostatus: 'zorgtoeslagbeschikking',
    params: { zaakkenmerk: `zorgtoeslag/${bsn}` },
  },
  {
    label: 'Uitbetaald',
    cell: 'belastingdienst',
    lexostatus: 'betaald_tot_nu_toe',
    params: { zaakkenmerk: `zorgtoeslag/${bsn}` },
  },
];

export const portaalFixture = {
  actor: 'burger',
  label: 'Aanvraagportaal',
  personas: [
    { id: 'aanvrager-a', label: 'Aanvrager A (fictief)', values: { bsn: '999993653', jaar: 2024 }, inzicht: vragen('999993653') },
    { id: 'aanvrager-b', label: 'Aanvrager B (fictief)', values: { bsn: '999990019', jaar: 2024 }, inzicht: vragen('999990019') },
  ],
  inzicht: vragen('{bsn}'),
};

/**
 * Het beeld met een gekozen persona, zoals de server het na `PUT /api/persona`
 * geeft: haar id erin, en de formulieren van de actor met haar waarden.
 */
export function worldAs(id) {
  const world = cloneWorld();
  world.persona = id;
  const persona = portaalFixture.personas.find((candidate) => candidate.id === id);
  if (!persona) return world;
  for (const action of world.actions) {
    if (action.actor === portaalFixture.actor) action.prefill = { ...action.prefill, ...persona.values };
  }
  return world;
}
