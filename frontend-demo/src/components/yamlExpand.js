/**
 * Welke knopen van de YAML-boom staan open als een wet net geopend is.
 *
 * De presentator klapt één artikel per keer open en vertelt erbij. Dat stelt
 * twee eisen die elkaar in de weg lijken te zitten: de boom moet kaal beginnen,
 * én één klik op een artikel moet meteen de juiste regels tonen in plaats van
 * de presentator nog vier niveaus dieper te laten klikken.
 *
 * Daarom is het artikel de enige handmatige stap, en ligt de weg naar een
 * geconfigureerd pad al open te wachten, tot en met dat pad zelf. Buren blijven
 * dicht omdat ze niet op die weg liggen, en wat onder het geconfigureerde pad
 * hangt blijft dicht omdat het pad daar ophoudt: dat laatste is wat een ondiep
 * pad als `…execution.actions` anders alsnog in één klap zou uitklappen.
 *
 * De paden komen uit `expanded_paths` in `corpus/demo/demo-config.yaml`.
 */

/** `articles.2` — het enige knooppunt dat de presentator zelf openklapt. */
const ARTICLE = /^articles\.[^.]+$/;

/**
 * Ligt `path` op de weg naar `pattern`, of ís het `pattern`? `*` matcht elk
 * segment.
 *
 * Wat eronder ligt telt níét mee. Een pad dat ondiep eindigt — de
 * precariobelasting stopt bij `…execution.actions` — zou anders zijn hele
 * deelboom tot elke diepte openzetten, en dat is precies het uitklappen-in-één-
 * keer waar deze stand vanaf moest.
 */
export function onPath(pattern, path) {
  const p = pattern.split('.');
  const q = path.split('.');
  if (q.length > p.length) return false;
  for (let i = 0; i < q.length; i += 1) {
    if (p[i] !== '*' && q[i] !== '*' && p[i] !== q[i]) return false;
  }
  return true;
}

/**
 * @param {{path: string, depth: number, all: boolean|null, paths: string[]}} node
 * @returns {boolean}
 */
export function initiallyOpen({ path, depth, all, paths }) {
  // De knoppen "alles open" en "alles dicht" overrulen de voorbereide stand.
  if (all === true) return true;
  if (all === false) return false;
  if (depth < 1) return true;
  // Eerder stond elk artikel én zijn machine_readable-blok hier
  // onvoorwaardelijk open, waardoor de hele boom in één keer uitklapte en er
  // niets meer te onthullen viel.
  if (ARTICLE.test(path)) return false;
  if (path === 'articles') return true;
  return (paths ?? []).some((pattern) => onPath(pattern, path));
}
