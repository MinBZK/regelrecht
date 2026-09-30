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

/** Every folder under the corpus roots that holds `<date>.yaml` law files, by $id (folder name). */
export function indexCorpus(corpusRoots) {
  const index = new Map();
  const walk = (dir, depth) => {
    if (depth > 7) return;
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    const dated = entries.filter((e) => e.isFile() && DATE_FILE_RE.test(e.name));
    if (dated.length && !index.has(path.basename(dir))) {
      index.set(path.basename(dir), { dir, versions: dated.map((e) => e.name.match(DATE_FILE_RE)[1]).sort() });
    }
    for (const e of entries) {
      if (e.isDirectory() && !e.name.startsWith('.') && e.name !== 'node_modules') walk(path.join(dir, e.name), depth + 1);
    }
  };
  for (const root of corpusRoots) walk(root, 0);
  return index;
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

/** The Vite plugin. Options: `decksRoot`, `corpusRoots` (absolute paths). */
export function presenterApi({ decksRoot, corpusRoots }) {
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
          const deck = path.relative(decksRoot, abs).split(path.sep)[0];
          server.ws.send({ type: 'custom', event: 'presenter:deck-changed', data: { deck } });
        } else if (corpusRoots.some((r) => abs.startsWith(path.resolve(r) + path.sep))) {
          corpus = null;
          server.ws.send({ type: 'custom', event: 'presenter:corpus-changed', data: {} });
        }
      };
      server.watcher.on('change', onChange);
      server.watcher.on('add', onChange);
      server.watcher.on('unlink', onChange);

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
          next();
        } catch (err) {
          send(res, err.status ?? 500, { error: err.message });
        }
      });
    },
  };
}
