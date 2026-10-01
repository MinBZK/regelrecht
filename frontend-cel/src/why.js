// The "waarom?" of the application form: the steps the cell gives per field
// and for the form (GET /api/form, `why`), and the YAML fragment behind a
// step. The texts of the steps come from the cell; this only builds the
// route of a fragment and fetches it once, when it is opened.

// The route under /processes/<id>/api of the fragment of a step's source, or
// null if there is nothing to open (also a law without `#<article>`). With
// `whole: true` the whole file the fragment is in.
export function fragmentPath(source) {
  if (source?.law) {
    const [regulation, article] = source.law.split('#');
    if (!regulation || !article) return null;
    const law = `/law/${encodeURIComponent(regulation)}`;
    return source.whole ? law : `${law}/${encodeURIComponent(article)}`;
  }
  if (source?.config) {
    const path = `/config/${source.config.split('/').map(encodeURIComponent).join('/')}`;
    return source.anchor && !source.whole ? `${path}?anchor=${encodeURIComponent(source.anchor)}` : path;
  }
  return null;
}

// Where a step is written, as a short label.
export function stepLabel(step) {
  const s = step.source ?? {};
  if (s.law) return s.law;
  if (s.config) return s.anchor ? `${s.config}: ${s.anchor}` : s.config;
  return '';
}

// Fetch a fragment per source once (`fetcher(path)`); a failure is not
// remembered.
export function fragmentCache(fetcher) {
  const cache = new Map();
  return (source) => {
    const path = fragmentPath(source);
    if (!path) return Promise.resolve(null);
    if (!cache.has(path)) {
      cache.set(
        path,
        fetcher(path).catch((e) => {
          cache.delete(path);
          throw e;
        }),
      );
    }
    return cache.get(path);
  };
}
