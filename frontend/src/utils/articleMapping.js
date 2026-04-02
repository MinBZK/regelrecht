/**
 * Builds a mapping from output/input/parameter names to article numbers.
 *
 * @param {Array} articles - Articles array from useLaw()
 * @returns {{ outputToArticle: Map<string, string>, inputToArticle: Map<string, string>, paramToArticle: Map<string, string> }}
 */
export function buildArticleMap(articles) {
  const outputToArticle = new Map();
  const inputToArticle = new Map();
  const paramToArticle = new Map();

  for (const article of articles || []) {
    const exec = article.machine_readable?.execution;
    if (!exec) continue;
    const num = String(article.number);
    for (const o of exec.output || []) outputToArticle.set(o.name, num);
    for (const i of exec.input || []) inputToArticle.set(i.name, num);
    for (const p of exec.parameters || []) paramToArticle.set(p.name, num);
  }

  return { outputToArticle, inputToArticle, paramToArticle };
}
