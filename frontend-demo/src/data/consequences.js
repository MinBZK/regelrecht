/**
 * Gevolgen: wat er buiten de overheid gebeurt door haar besluiten. Per partij
 * die geen overheid is (een cel onder `consequences` in demo-config.yaml) wat
 * die cel over de persona heeft vastgelegd: bij een bank de rekening, het
 * saldo en wat zij bijschreef of weigerde.
 *
 * Generiek: welke partij het is en welke velden zij gebruikt, staat in de
 * configuratie; hoe een soort partij wordt getoond, zegt haar `view`. Een view
 * die de demo niet kent, toont niets in plaats van iets te verzinnen.
 */
import { accountOf } from './account.js';

/** Per `view` wat de demo uit de gegevens en de kroniek leest. */
const VIEWS = {
  account: accountOf,
};

/**
 * De gevolgen voor de persona met `sources` (zijn gegevens per organisatie en
 * tabel), per partij in `config` (`consequences` in demo-config.yaml), met de
 * grammen `grams` van alle cellen. Per partij `{ cell, view, service, data }`,
 * met `service` de organisatie waaronder de cel valt (`serviceOf(cellId)`) en
 * `data` wat haar view gaf; `null` als de persona daar niets heeft.
 */
export function consequencesOf(config, { sources, grams, cells = [], serviceOf = () => null }) {
  return (config ?? []).map((party) => {
    const read = VIEWS[party.view];
    return {
      cell: party.cell,
      view: party.view ?? null,
      service: serviceOf(party.cell),
      data: read ? read(party, sources, grams, cells) : null,
    };
  });
}
