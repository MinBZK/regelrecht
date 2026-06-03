/**
 * Group a flat `articles[]` array into per-article groups.
 *
 * Harvested laws can be split down to leaf level (lid → onderdeel → sub),
 * producing many `articles[]` entries for a single actual article — e.g.
 * `besluit_zorgverzekering` yields separate entries `1.e.1°`, `2.4.1.a.1°`.
 * The article list (library column 2) should show ONE item per article and
 * load all underlying segments together when selected.
 *
 * The reliable grouping key is the `#…` fragment of each entry's `url`, which
 * is the actual article anchor on wetten.overheid.nl. Every segment of an
 * article shares it (`1.e.1°` and `1.f` both carry `#Artikel1`; `2.4.1` and
 * `2.4.1.a.1°` both carry `#Artikel2.4`). This disambiguates dotted numbers
 * that the number string alone cannot: `2.1.1` (article 2.1, lid 1) groups
 * under `#Artikel2.1`, while a genuine dotted article `3.1` (`#Artikel3.1`)
 * stays its own group instead of being folded into "article 3".
 */

/** Extract the article anchor (the part after `#`) from an entry's url. */
export function articleAnchor(url) {
  if (typeof url !== "string") return null;
  const hash = url.indexOf("#");
  if (hash < 0) return null;
  const fragment = url.slice(hash + 1).trim();
  return fragment.length > 0 ? fragment : null;
}

/**
 * Derive the article number used in routes/labels from an anchor.
 * `Artikel2.4` → `2.4`, `ArtikelAanhef` → `Aanhef`. Anchors that don't follow
 * the `Artikel<n>` convention are returned as-is.
 */
function numberFromAnchor(anchor) {
  const m = /^Artikel(.+)$/.exec(anchor);
  return m ? m[1] : anchor;
}

/** Human label for the article list item. */
function labelForNumber(number) {
  // The "Aanhef" (preamble) reads oddly as "Artikel Aanhef"; show it bare.
  return /^\d/.test(String(number)) ? `Artikel ${number}` : String(number);
}

/**
 * Convert a flat articles array into ordered groups.
 *
 * @param {Array} articles - the law's `articles[]` (each: { number, text, url, machine_readable? })
 * @returns {Array<{ key: string, number: string, label: string, segments: Array }>}
 *          Groups in order of first appearance; segments in YAML order.
 */
export function groupArticles(articles) {
  const groups = [];
  const byKey = new Map();

  for (const article of articles || []) {
    const anchor = articleAnchor(article.url);
    // Anchor-keyed groups share the anchor; fragmentless entries stand alone
    // under a number-namespaced key so they can't collide with an anchor.
    const number = anchor ? numberFromAnchor(anchor) : String(article.number);
    const key = anchor ? `#${anchor}` : `n:${number}`;

    let group = byKey.get(key);
    if (!group) {
      group = { key, number, label: labelForNumber(number), segments: [] };
      byKey.set(key, group);
      groups.push(group);
    }
    group.segments.push(article);
  }

  return groups;
}

/**
 * Find the group a given article number belongs to. Matches the group's
 * representative `number` first (new article-level routes), then falls back to
 * any segment number it contains (legacy deep-links to a leaf like `1.e.1°`).
 *
 * @returns the matching group, or undefined.
 */
export function findGroupForArticleNumber(groups, number) {
  if (number == null) return undefined;
  const target = String(number);
  return (
    groups.find((g) => String(g.number) === target) ??
    groups.find((g) => g.segments.some((s) => String(s.number) === target))
  );
}
