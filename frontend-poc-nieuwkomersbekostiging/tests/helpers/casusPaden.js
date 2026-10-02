// Waar de casus-inhoud staat, vanuit de tests gezien.
//
// In de oorspronkelijke PoC-repo lag `corpus/` en `data/` één map boven de app
// (`<casus>/app/tests` → `<casus>/corpus`). In de monorepo staat de app in
// `frontend-poc-nieuwkomersbekostiging/` en de inhoud in
// `corpus-poc/nieuwkomersbekostiging/`.
//
// Dat verschil is niet onschuldig: `resolve(__dirname, '..', '..', 'corpus')`
// wees hier naar het échte regelrecht-corpus, waarna de scenariotests vrolijk
// de zorgtoeslag en het Burgerlijk Wetboek gingen draaien tegen een engine die
// alleen studiefinanciering geladen had. Vijfenveertig rode tests, geen enkele
// over deze casus.
import { dirname, resolve } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

/** De app zelf (frontend-poc-nieuwkomersbekostiging/). */
export const appRoot = resolve(__dirname, '..', '..');

/** De casus-inhoud (corpus-poc/nieuwkomersbekostiging/). */
export const casusRoot = resolve(appRoot, '..', 'corpus-poc', 'nieuwkomersbekostiging');

export const corpusDir = resolve(casusRoot, 'corpus');
export const dataDir = resolve(casusRoot, 'data');
