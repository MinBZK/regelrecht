/**
 * The presenter's file API: a Vite dev-server middleware, so there is no
 * second process to start. It reads a folder of decks (one folder per deck,
 * one markdown file per slide), writes edits made in the browser back to
 * those files, and looks up law articles in the corpus for ```wet blocks.
 *
 * Dev-only on purpose: the files on disk are the source, and a hosted build
 * has no disk to write to.
 */
import fs from 'node:fs';
import path from 'node:path';
import * as yaml from 'js-yaml';

const SLIDE_RE = /^[^/\\]+\.md$/;
const DATE_FILE_RE = /^(\d{4}-\d{2}-\d{2})\.yaml$/;
const LAW_FILE_RE = /^[^/\\]+\.ya?ml$/;

export class ApiError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

/**
 * The absolute path of a file inside a deck, or an ApiError. Only slide files
 * (`*.md`) and `deck.yaml` may be read or written, and the resolved path has to
 * stay inside the deck folder: a name like `../../x.md` or an absolute path is
 * refused, not normalised.
 */
export function resolveDeckFile(decksRoot, deck, file) {
  const deckDir = resolveDeckDir(decksRoot, deck);
  if (typeof file !== 'string' || (file !== 'deck.yaml' && !SLIDE_RE.test(file))) {
    throw new ApiError(400, `Geen dia-bestand: ${file}`);
  }
  const full = path.resolve(deckDir, file);
  if (path.dirname(full) !== deckDir) throw new ApiError(400, `Pad buiten de deck-map: ${file}`);
  return full;
}

export function resolveDeckDir(decksRoot, deck) {
  const root = path.resolve(decksRoot);
  if (typeof deck !== 'string' || !deck || deck.startsWith('.') || /[/\\]/.test(deck)) {
    throw new ApiError(400, `Ongeldige deck-naam: ${deck}`);
  }
  const dir = path.resolve(root, deck);
  if (path.dirname(dir) !== root) throw new ApiError(400, `Deck buiten de decks-map: ${deck}`);
  return dir;
}

export function listDecks(decksRoot) {
  if (!fs.existsSync(decksRoot)) return [];
  return fs
    .readdirSync(decksRoot, { withFileTypes: true })
    .filter((d) => d.isDirectory() && !d.name.startsWith('.'))
    .map((d) => {
      const meta = readDeckMeta(path.join(decksRoot, d.name));
      const slides = fs.readdirSync(path.join(decksRoot, d.name)).filter((f) => SLIDE_RE.test(f)).length;
      return { name: d.name, title: meta.title ?? d.name, slides };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
}

/** deck.yaml parsed; a broken one reads as empty so the deck can still be opened and fixed. */
function readDeckMeta(deckDir) {
  const file = path.join(deckDir, 'deck.yaml');
  if (!fs.existsSync(file)) return {};
  try {
    const meta = yaml.load(fs.readFileSync(file, 'utf8'));
    return meta && typeof meta === 'object' ? meta : {};
  } catch {
    return {};
  }
}

/** A deck: its deck.yaml (raw and parsed) and every slide file, sorted by name. */
export function readDeck(decksRoot, deck) {
  const dir = resolveDeckDir(decksRoot, deck);
  if (!fs.existsSync(dir)) throw new ApiError(404, `Deck niet gevonden: ${deck}`);
  const metaFile = path.join(dir, 'deck.yaml');
  const hasMeta = fs.existsSync(metaFile);
  const slides = fs
    .readdirSync(dir)
    .filter((f) => SLIDE_RE.test(f))
    .sort(compareSlides)
    .map((file) => {
      const full = path.join(dir, file);
      return { file, content: fs.readFileSync(full, 'utf8'), mtime: fs.statSync(full).mtimeMs };
    });
  return {
    name: deck,
    meta: hasMeta ? readDeckMeta(dir) : {},
    metaRaw: hasMeta ? fs.readFileSync(metaFile, 'utf8') : '',
    metaMtime: hasMeta ? fs.statSync(metaFile).mtimeMs : null,
    slides,
  };
}

/**
 * Write a deck file. `mtime` is the modification time the client read; when
 * the file changed on disk since (someone edited it in their editor), the
 * write is refused with 409 instead of silently overwriting that edit. A
 * `null` mtime means "I expect the file not to exist yet".
 */
export function writeDeckFile(decksRoot, deck, file, content, mtime) {
  const full = resolveDeckFile(decksRoot, deck, file);
  if (typeof content !== 'string') throw new ApiError(400, 'content ontbreekt');
  const exists = fs.existsSync(full);
  const current = exists ? fs.statSync(full).mtimeMs : null;
  if (mtime !== undefined && current !== mtime) {
    throw new ApiError(409, `${file} is intussen op schijf gewijzigd; herlaad de dia`);
  }
  fs.writeFileSync(full, content, 'utf8');
  return { file, mtime: fs.statSync(full).mtimeMs };
}

/** The order of slide files: by name without `.md`, numbers numerically. */
export function compareSlides(a, b) {
  return a.replace(/\.md$/, '').localeCompare(b.replace(/\.md$/, ''), 'nl', { numeric: true });
}

const NEW_SLIDE = '---\nkind: content\n---\n\n# Nieuwe dia\n\nTekst\n';
const NAME_CHARS = '0123456789abcdefghijklmnopqrstuvwxyz';

/**
 * A new, empty slide right after `after` (a slide file, or null for the end),
 * without renaming any other file: the name is `after`'s name plus a suffix,
 * chosen so it sorts between `after` and the slide that follows it. Of the
 * names that fit, the middle one is taken, which leaves room for the next
 * insertion on either side.
 */
export function createSlide(decksRoot, deck, after) {
  const dir = resolveDeckDir(decksRoot, deck);
  const slides = fs.readdirSync(dir).filter((f) => SLIDE_RE.test(f)).sort(compareSlides);
  if (after != null && (!SLIDE_RE.test(after) || !slides.includes(after))) throw new ApiError(400, `Geen dia in dit deck: ${after}`);
  const prev = after ?? slides[slides.length - 1] ?? null;
  const next = prev ? slides[slides.indexOf(prev) + 1] ?? null : null;
  if (!prev) return writeDeckFile(decksRoot, deck, '01-titel.md', NEW_SLIDE, null);
  const base = prev.replace(/\.md$/, '');
  const suffixes = [];
  for (const a of NAME_CHARS) {
    suffixes.push(a);
    for (const b of NAME_CHARS) suffixes.push(a + b);
  }
  const fits = suffixes
    .map((sfx) => `${base}-${sfx}.md`)
    .filter((f) => !slides.includes(f) && compareSlides(f, prev) > 0 && (!next || compareSlides(f, next) < 0))
    .sort(compareSlides);
  if (!fits.length) throw new ApiError(409, 'Geen vrije bestandsnaam tussen deze dia en de volgende; hernoem de bestanden');
  return writeDeckFile(decksRoot, deck, fits[Math.floor(fits.length / 2)], NEW_SLIDE, null);
}

/**
 * Every folder under the corpus roots that holds `<date>.yaml` law files, by
 * the law's `$id`. The `$id` is read from the file, not taken from the folder
 * name: most laws sit in a folder of that name, but not all (a municipal
 * regulation lives under `gemeentelijke_verordening/amsterdam/apv_erfgrens/`
 * with `$id: apv_erfgrens_amsterdam`). The folder name is kept as an alias for
 * the wet block, when it differs and names no other law. An earlier root wins
 * on a clash, so corpus/regulation comes before corpus-poc.
 */
export function indexCorpus(corpusRoots) {
  const index = new Map();
  const aliases = [];
  const walk = (dir, depth, root) => {
    if (depth > 7) return;
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    const dated = entries.filter((e) => e.isFile() && DATE_FILE_RE.test(e.name));
    if (dated.length) {
      const versions = dated.map((e) => e.name.match(DATE_FILE_RE)[1]).sort();
      const id = readId(path.join(dir, `${versions[versions.length - 1]}.yaml`)) ?? path.basename(dir);
      if (!index.has(id)) index.set(id, { id, dir, versions, root });
      if (id !== path.basename(dir)) aliases.push([path.basename(dir), id]);
    }
    for (const e of entries) {
      if (e.isDirectory() && !e.name.startsWith('.') && e.name !== 'node_modules') walk(path.join(dir, e.name), depth + 1, root);
    }
  };
  for (const root of corpusRoots) walk(root, 0, root);
  for (const [alias, id] of aliases) if (!index.has(alias)) index.set(alias, index.get(id));
  return index;
}

/** `$id` of a law file, read from its top lines without parsing the whole YAML. */
function readId(file) {
  try {
    const head = fs.readFileSync(file, 'utf8').slice(0, 4096);
    return head.match(/^\$id:\s*['"]?([^'"\s#]+)/m)?.[1] ?? null;
  } catch {
    return null;
  }
}

/** The newest version on or before `date` (ISO), or the oldest one when all are later. */
export function pickVersion(versions, date) {
  const on = versions.filter((v) => v <= date);
  return on.length ? on[on.length - 1] : versions[0];
}

export function lookupArticle(index, { law, article, date }) {
  const entry = index.get(law);
  if (!entry) throw new ApiError(404, `Wet niet gevonden in de corpus: ${law}`);
  const version = pickVersion(entry.versions, date || new Date().toISOString().slice(0, 10));
  const file = path.join(entry.dir, `${version}.yaml`);
  return articleOf(parseLaw(file, `${law} (${version})`), article, law, version);
}

/**
 * A law YAML placed in the deck folder next to the slides, for a variant or a
 * bill that is not in the corpus. Same rules as the slide files: a plain name
 * inside the deck folder, and not deck.yaml.
 */
export function resolveDeckLaw(decksRoot, deck, file) {
  const deckDir = resolveDeckDir(decksRoot, deck);
  if (typeof file !== 'string' || !LAW_FILE_RE.test(file) || file === 'deck.yaml') {
    throw new ApiError(400, `Geen wet-bestand: ${file}`);
  }
  const full = path.resolve(deckDir, file);
  if (path.dirname(full) !== deckDir) throw new ApiError(400, `Pad buiten de deck-map: ${file}`);
  if (!fs.existsSync(full)) throw new ApiError(404, `${file} staat niet in de deck-map`);
  return full;
}

export function lookupDeckArticle(decksRoot, { deck, file, article }) {
  const full = resolveDeckLaw(decksRoot, deck, file);
  const doc = parseLaw(full, file);
  return articleOf(doc, article, doc?.$id ?? file, doc?.valid_from ?? file);
}

function parseLaw(file, label) {
  try {
    const doc = yaml.load(fs.readFileSync(file, 'utf8'));
    if (!doc || typeof doc !== 'object') throw new Error('geen YAML-document');
    return doc;
  } catch (e) {
    throw new ApiError(422, `${label} is geen geldige YAML: ${e.reason ?? e.message}`);
  }
}

function articleOf(doc, article, law, version) {
  const art = (doc.articles ?? []).find((a) => String(a.number) === String(article));
  if (article != null && article !== '' && !art) throw new ApiError(404, `Artikel ${article} niet gevonden in ${law} (${version})`);
  return {
    law: {
      id: doc.$id ?? law,
      name: typeof doc.name === 'string' && !doc.name.startsWith('#') ? doc.name : null,
      valid_from: doc.valid_from ?? version,
      regulatory_layer: doc.regulatory_layer ?? null,
      url: doc.url ?? null,
    },
    article: art ?? null,
  };
}

const FEATURE_RE = /^[^/\\]+\.feature$/;

/**
 * A scenario file. `law/file.feature` is a file in that law's `scenarios/`
 * folder in the corpus; a bare `file.feature` is a file in the deck folder,
 * for a case of your own.
 */
export function readFeature(index, decksRoot, { feature, deck }) {
  if (typeof feature !== 'string' || !feature) throw new ApiError(400, 'scenario ontbreekt');
  const parts = feature.split('/');
  let full;
  if (parts.length === 1) {
    if (!FEATURE_RE.test(feature)) throw new ApiError(400, `Geen feature-bestand: ${feature}`);
    const deckDir = resolveDeckDir(decksRoot, deck);
    full = path.join(deckDir, feature);
  } else if (parts.length === 2) {
    const [law, file] = parts;
    if (!/^[A-Za-z0-9_-]+$/.test(law)) throw new ApiError(400, `Geen wet-id: ${law}`);
    if (!FEATURE_RE.test(file)) throw new ApiError(400, `Geen feature-bestand: ${file}`);
    const entry = index.get(law);
    if (!entry) throw new ApiError(404, `Wet niet gevonden in de corpus: ${law}`);
    full = path.join(entry.dir, 'scenarios', file);
  } else {
    throw new ApiError(400, `Schrijf het scenario als wet/bestand.feature of bestand.feature: ${feature}`);
  }
  if (!fs.existsSync(full)) throw new ApiError(404, `Scenario-bestand niet gevonden: ${feature}`);
  return { feature, text: fs.readFileSync(full, 'utf8') };
}

/** The law YAMLs in a deck folder with their `$id`, in name order. */
function deckLaws(decksRoot, deck) {
  if (!deck) return [];
  const deckDir = resolveDeckDir(decksRoot, deck);
  if (!fs.existsSync(deckDir)) return [];
  return fs
    .readdirSync(deckDir)
    .sort()
    .filter((f) => LAW_FILE_RE.test(f) && f !== 'deck.yaml')
    .map((file) => ({ file, id: readId(path.join(deckDir, file)), text: fs.readFileSync(path.join(deckDir, file), 'utf8') }))
    .filter((l) => l.id);
}

const corpusVersions = (entry) =>
  entry.versions.map((v) => ({ law: entry.id, source: `corpus ${v}`, text: fs.readFileSync(path.join(entry.dir, `${v}.yaml`), 'utf8') }));

/**
 * The laws for a reken block's engine, the way the Rust BDD runner loads them:
 * every law of the main corpus (the first root), each in every version, so a
 * law that implements, overrides or is pulled in by the law under test is
 * there without the scenario naming it. The engine picks the version by
 * calculation date; filtering on date here would drop versions it needs.
 * The deck's own law YAMLs come last, so a variant replaces the corpus
 * version with the same `valid_from`.
 */
export function readAllLaws(index, decksRoot, { deck, root }) {
  const seen = new Set();
  const laws = [];
  for (const entry of index.values()) {
    if (entry.root !== root || seen.has(entry.id)) continue;
    seen.add(entry.id);
    laws.push(...corpusVersions(entry));
  }
  for (const l of deckLaws(decksRoot, deck)) laws.push({ law: l.id, source: l.file, text: l.text });
  return { versions: laws };
}

/**
 * One law in every version, for a law outside the main corpus that a
 * scenario loads by name (a corpus-poc law, or one from PRESENTER_CORPUS),
 * followed by the deck's own YAMLs with that `$id`.
 */
export function readLawVersions(index, decksRoot, { law, deck }) {
  const entry = index.get(law);
  const versions = entry ? corpusVersions(entry) : [];
  for (const l of deckLaws(decksRoot, deck)) if (l.id === law) versions.push({ law, source: l.file, text: l.text });
  if (!versions.length) throw new ApiError(404, `Wet niet gevonden: ${law}`);
  return { law, versions };
}

const WASM_FILES = { 'regelrecht_engine.js': 'text/javascript', 'regelrecht_engine_bg.wasm': 'application/wasm' };

/**
 * The JSON body of a request. Anything but `application/json` is refused: a
 * page on another site can send a form POST to localhost without a preflight,
 * but not with that content type.
 */
function readBody(req) {
  if (!/^application\/json\b/i.test(req.headers['content-type'] ?? '')) {
    return Promise.reject(new ApiError(415, 'Alleen application/json'));
  }
  return new Promise((resolve, reject) => {
    let data = '';
    req.on('data', (c) => (data += c));
    req.on('end', () => {
      try {
        resolve(data ? JSON.parse(data) : {});
      } catch {
        reject(new ApiError(400, 'Ongeldige JSON'));
      }
    });
    req.on('error', reject);
  });
}

function send(res, status, body) {
  res.statusCode = status;
  res.setHeader('Content-Type', 'application/json; charset=utf-8');
  res.end(JSON.stringify(body));
}

/**
 * The Vite plugin. Options: `decksRoot`, `corpusRoots` and `wasmDir`
 * (absolute paths); `wasmDir` holds the engine that `just wasm-build` makes.
 */
export function presenterApi({ decksRoot, corpusRoots, wasmDir }) {
  let corpus = null;
  const getCorpus = () => (corpus ??= indexCorpus(corpusRoots));

  return {
    name: 'presenter-api',
    configureServer(server) {
      // Watch the decks folder so an edit in another editor reloads the slide.
      // The corpus index is rebuilt lazily when a law file changes.
      server.watcher.add([decksRoot, ...corpusRoots]);
      const onChange = (file) => {
        const abs = path.resolve(file);
        if (abs.startsWith(path.resolve(decksRoot) + path.sep)) {
          const [deck, ...rest] = path.relative(decksRoot, abs).split(path.sep);
          server.ws.send({ type: 'custom', event: 'presenter:deck-changed', data: { deck, file: rest.join('/') } });
        } else if (corpusRoots.some((r) => abs.startsWith(path.resolve(r) + path.sep))) {
          corpus = null;
          server.ws.send({ type: 'custom', event: 'presenter:corpus-changed', data: {} });
        }
      };
      server.watcher.on('change', onChange);
      server.watcher.on('add', onChange);
      server.watcher.on('unlink', onChange);

      // The engine, served from the build output instead of copied: two copies
      // of a 2 MB build drift, and `just wasm-build` already writes this one.
      server.middlewares.use('/wasm/pkg', (req, res, next) => {
        const name = new URL(req.url, 'http://x').pathname.replace(/^\//, '');
        if (!Object.hasOwn(WASM_FILES, name)) return next();
        const full = path.join(wasmDir, name);
        if (!fs.existsSync(full)) {
          return send(res, 404, { error: `De engine is nog niet gebouwd (${name} ontbreekt). Draai eerst: just wasm-build` });
        }
        res.setHeader('Content-Type', WASM_FILES[name]);
        res.setHeader('Cache-Control', 'no-cache');
        fs.createReadStream(full).pipe(res);
      });

      server.middlewares.use('/api', async (req, res, next) => {
        try {
          const url = new URL(req.url, 'http://x');
          const parts = url.pathname.split('/').filter(Boolean).map(decodeURIComponent);
          if (parts[0] === 'decks') {
            if (req.method === 'GET' && parts.length === 1) return send(res, 200, listDecks(decksRoot));
            if (req.method === 'GET' && parts.length === 2) return send(res, 200, readDeck(decksRoot, parts[1]));
            if (req.method === 'POST' && parts.length === 2) {
              const body = await readBody(req);
              return send(res, 201, createSlide(decksRoot, parts[1], body.after ?? null));
            }
            if (req.method === 'PUT' && parts.length === 3) {
              const body = await readBody(req);
              return send(res, 200, writeDeckFile(decksRoot, parts[1], parts[2], body.content, body.mtime));
            }
          }
          if (parts[0] === 'wet' && req.method === 'GET') {
            const q = url.searchParams;
            if (q.get('file')) {
              return send(res, 200, lookupDeckArticle(decksRoot, { deck: q.get('deck'), file: q.get('file'), article: q.get('article') }));
            }
            return send(res, 200, lookupArticle(getCorpus(), { law: q.get('law'), article: q.get('article'), date: q.get('date') }));
          }
          if (parts[0] === 'scenario' && req.method === 'GET') {
            const q = url.searchParams;
            return send(res, 200, readFeature(getCorpus(), decksRoot, { feature: q.get('feature'), deck: q.get('deck') }));
          }
          if (parts[0] === 'laws' && parts.length === 1 && req.method === 'GET') {
            return send(res, 200, readAllLaws(getCorpus(), decksRoot, { deck: url.searchParams.get('deck'), root: corpusRoots[0] }));
          }
          if (parts[0] === 'laws' && parts.length === 2 && req.method === 'GET') {
            return send(res, 200, readLawVersions(getCorpus(), decksRoot, { law: parts[1], deck: url.searchParams.get('deck') }));
          }
          next();
        } catch (err) {
          send(res, err.status ?? 500, { error: err.message });
        }
      });
    },
  };
}
