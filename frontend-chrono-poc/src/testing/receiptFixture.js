/**
 * Het antwoord van de receipt-route, zoals de server het geeft.
 *
 * Met de hand geschreven en niet — zoals `worldFixture` — het bestand dat de
 * crate zelf vastpint, en dat heeft precies de reden die deze hele route heeft:
 * een receipt draagt **wandkloktijd**. Het verschilt per run, dus er valt niets
 * van vast te pinnen dat twee keer hetzelfde is. Dit is daarom de *vorm* van het
 * antwoord, overgenomen van een echte run over de publieke wereld, met de
 * tijdstempel op een vaste waarde.
 *
 * De vorm zelf wordt aan de Rust-kant bewaakt (`packages/simulator/tests/receipt.rs`
 * en `packages/chrono-poc-web/tests/api.rs`): die tests draaien over de echte
 * wereld en worden rood als een sectie verdwijnt of anders gaat heten.
 */
export const receiptFixture = {
  gram: {
    cell: 'toeslagen',
    chronicle: 'beschikkingen',
    index: 0,
    name: 'zorgtoeslag_toekenning',
    besluit: 'zorgtoeslag_toekenning',
    zaakkenmerk: 'zorgtoeslag/999993653',
    op_moment: '2024-03-01',
  },
  engine_config: {
    connectivity: 'solo',
    legal_status: 'simulation',
    untranslatable_mode: 'error',
  },
  execution: {
    calculation_date: '2024-03-01',
    parameters: { bsn: '999993653', is_verzekerde: true, toetsingsinkomen: 81000 },
  },
  provenance: {
    engine: 'regelrecht',
    engine_version: '0.3.0',
    regulation_hash: 'sha256:550334d7acca33aadf30abe9a50442eb3a0e321dc0b9097b6864459f1459a33e',
    regulation_id: 'wet_op_de_zorgtoeslag',
    regulation_valid_from: '2024-01-01',
    schema_version: 'v0.5.0',
  },
  results: {
    output_provenance: {
      heeft_recht_op_zorgtoeslag: { article: '2', law_id: 'wet_op_de_zorgtoeslag', type: 'Direct' },
    },
    outputs: { heeft_recht_op_zorgtoeslag: true, hoogte_zorgtoeslag: 197178.01 },
    requested_outputs: ['heeft_recht_op_zorgtoeslag', 'hoogte_zorgtoeslag'],
    // De uitvoeringstrace, ingekort tot de vormen die een weergave moet aankunnen:
    // een wortel over de hele uitvoering, een sprong naar een andere regeling, een
    // artikel dat rekent (mét regeling en artikelnummer), een bewerking eronder en
    // een waarde die ergens vandaan opgelost is. Geen looptijden: de engine schrijft
    // ze voor een gram niet mee, want ze verschillen per run.
    trace: {
      node_type: 'article',
      name: 'wet_op_de_zorgtoeslag (heeft_recht_op_zorgtoeslag, hoogte_zorgtoeslag)',
      message:
        'wet_op_de_zorgtoeslag (2024-03-01 {bsn: 999993653} heeft_recht_op_zorgtoeslag, hoogte_zorgtoeslag)',
      result: { heeft_recht_op_zorgtoeslag: true, hoogte_zorgtoeslag: 197178.01 },
      children: [
        {
          node_type: 'cross_law_reference',
          name: 'algemene_wet_inkomensafhankelijke_regelingen#heeft_toeslagpartner',
          children: [
            {
              node_type: 'action',
              name: 'heeft_toeslagpartner',
              regulation: 'algemene_wet_inkomensafhankelijke_regelingen',
              article: '3',
              message: 'Computing heeft_toeslagpartner',
              result: false,
            },
          ],
        },
        {
          node_type: 'action',
          name: 'hoogte_zorgtoeslag',
          regulation: 'wet_op_de_zorgtoeslag',
          article: '3',
          message: 'Computing hoogte_zorgtoeslag',
          result: 197178.01,
          children: [
            {
              node_type: 'operation',
              name: 'SUBTRACT',
              result: 197178.01,
              children: [
                {
                  node_type: 'resolve',
                  name: 'toetsingsinkomen',
                  resolve_type: 'PARAMETER',
                  result: 81000,
                },
              ],
            },
          ],
        },
      ],
    },
  },
  scope: {
    loaded_regulations: [
      {
        hash: 'sha256:550334d7acca33aadf30abe9a50442eb3a0e321dc0b9097b6864459f1459a33e',
        id: 'wet_op_de_zorgtoeslag',
        valid_from: '2024-01-01',
      },
      {
        hash: 'sha256:b23b79d2b2125a95737141dc9c7fa6bb8e078e85ffbd415220c4afe249efd04b',
        id: 'algemene_wet_inkomensafhankelijke_regelingen',
      },
    ],
    sources: [],
  },
  accepted_values: [
    {
      output: 'toetsingsinkomen',
      value: 81000,
      cell: 'belastingdienst',
      authority: 'Belastingdienst',
      lexostatus: 'toetsingsinkomen',
      field: 'toetsingsinkomen',
      op_moment: '2024-03-01',
      zaakkenmerk: 'zorgtoeslag/999993653',
      asked_by: 'cel:toeslagen',
      signature: 'GESIMULEERDE-ONDERTEKENING door cel:toeslagen',
    },
  ],
  timestamp: {
    wall_clock: '2024-09-15T13:38:43.667205774+00:00',
    note: 'wandkloktijd van de uitvoering, niet de logische tijd van de wereld; daarom draagt het beeld van de wereld dit receipt niet',
  },
};

/** Een eigen exemplaar, voor een test die het antwoord aanpast. */
export function cloneReceipt() {
  return structuredClone(receiptFixture);
}
