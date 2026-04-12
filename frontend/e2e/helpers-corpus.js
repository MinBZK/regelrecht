/**
 * Shared corpus mock helper for Playwright e2e specs.
 *
 * Both `edit-test-loop.spec.js` (YAML-pane edit flow) and
 * `machine-panel-edit-loop.spec.js` (form-driven edit flow) need to fake
 * the editor-api's corpus endpoints so the spec can run without booting
 * Postgres + the Rust backend. Keeping the mock setup in one place
 * prevents the two specs from drifting and makes it cheap to add new
 * specs that exercise the same dependency graph.
 */
import { readFileSync, readdirSync, statSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import yaml from 'js-yaml';

const __dirname = dirname(fileURLToPath(import.meta.url));

/**
 * Absolute path to the on-disk corpus the helper reads law fixtures from.
 * Resolved relative to this file so callers don't need to know about it.
 */
export const CORPUS_ROOT = resolve(__dirname, '../../corpus/regulation/nl');

/**
 * Recursively walk a corpus directory and pick one YAML per `$id`,
 * preferring the latest publication date. Returns a Map<lawId,
 * { content, path, pubDate }>.
 *
 * NOTE: this picks by `publication_date` (lexicographic compare),
 * whereas the editor-api's `SourceMap::pick_best_version` selects by
 * `valid_from` against today's date. The two will agree as long as
 * each law in the test corpus has only one dated file (the case for
 * zorgtoeslagwet today). If a future test fixture introduces multiple
 * dated files for the same `$id`, align this with the server logic
 * to avoid the spec serving a different version than the editor would
 * see in production.
 */
export function loadCorpus(rootDir = CORPUS_ROOT) {
  const byId = new Map();

  function visit(dir) {
    for (const entry of readdirSync(dir)) {
      const full = resolve(dir, entry);
      if (statSync(full).isDirectory()) {
        visit(full);
      } else if (entry.endsWith('.yaml')) {
        const content = readFileSync(full, 'utf-8');
        const idMatch = content.match(/^\$id:\s*['"]?([^'"\n]+)['"]?$/m);
        if (!idMatch) continue;
        const lawId = idMatch[1].trim();
        const pubMatch = content.match(/^publication_date:\s*['"]?([^'"\n]+)['"]?$/m);
        const pubDate = pubMatch ? pubMatch[1].trim() : '';
        const existing = byId.get(lawId);
        if (!existing || pubDate > existing.pubDate) {
          byId.set(lawId, { content, path: full, pubDate });
        }
      }
    }
  }

  visit(rootDir);
  return byId;
}

/**
 * Find a scenario .feature file next to a law's YAML on disk. Returns the
 * raw text or null when no scenarios directory exists for that law.
 */
export function loadScenario(lawPath, filename) {
  const scenariosDir = resolve(dirname(lawPath), 'scenarios');
  try {
    return readFileSync(resolve(scenariosDir, filename), 'utf-8');
  } catch {
    return null;
  }
}

/**
 * Set up route intercepts so the editor frontend can fetch any law in the
 * corpus without a running editor-api. Also stubs the scenarios list/get
 * and the PUT save endpoint.
 *
 * Playwright runs route handlers in reverse registration order (LIFO), so
 * the more specific `/api/corpus/laws/*` routes registered later take
 * precedence over the bare `/api/corpus/laws` list handler.
 *
 * @param {import('@playwright/test').Page} page
 * @param {Map<string, {content: string, path: string}>} corpus - from loadCorpus()
 * @param {{id: string, scenarioFilename: string}} scenarioLaw - which law's scenarios to expose
 * @param {string} scenarioFile - raw .feature text returned by the scenario fetch
 */
export async function mockCorpusApi(page, corpus, scenarioLaw, scenarioFile) {
  // GET /api/corpus/laws — list for dependency discovery (fallback handler).
  await page.route('**/api/corpus/laws*', (route, request) => {
    const url = new URL(request.url());
    if (url.pathname !== '/api/corpus/laws') {
      return route.fallback();
    }
    const entries = [...corpus.entries()].map(([law_id, entry]) => {
      const displayName = resolveDisplayName(entry.content);
      return {
        law_id,
        name: null,
        display_name: displayName,
        source_id: 'local',
        source_name: 'Local Test Corpus',
      };
    });
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(entries),
    });
  });

  // GET /api/corpus/laws/{law_id}/outputs — list outputs from all articles.
  await page.route('**/api/corpus/laws/*/outputs', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(/\/api\/corpus\/laws\/([^/]+)\/outputs$/);
    if (!match) return route.fallback();
    const lawId = decodeURIComponent(match[1]);
    const entry = corpus.get(lawId);
    if (!entry) {
      return route.fulfill({ status: 404, body: `Law '${lawId}' not found` });
    }
    const outputs = collectOutputs(entry.content);
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(outputs),
    });
  });

  // GET /api/corpus/laws/{law_id} — serve from the corpus map.
  // PUT /api/corpus/laws/{law_id} — no-op; useLaw.saveLaw() updates rawYaml
  // locally on success so the test doesn't need a real backend write.
  await page.route('**/api/corpus/laws/*', (route, request) => {
    const url = new URL(request.url());
    const pathname = url.pathname;
    if (pathname.includes('/scenarios') || pathname.includes('/outputs')) {
      return route.fallback();
    }
    const lawId = decodeURIComponent(pathname.split('/').pop());
    if (request.method() === 'PUT') {
      return route.fulfill({ status: 200, body: '' });
    }
    const entry = corpus.get(lawId);
    if (!entry) {
      return route.fulfill({ status: 404, body: `Law '${lawId}' not found` });
    }
    return route.fulfill({
      status: 200,
      contentType: 'text/yaml; charset=utf-8',
      body: entry.content,
    });
  });

  // GET /api/corpus/laws/{law_id}/scenarios — list, only for the target law.
  await page.route('**/api/corpus/laws/*/scenarios', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(/\/api\/corpus\/laws\/([^/]+)\/scenarios$/);
    if (!match) return route.fallback();
    const lawId = decodeURIComponent(match[1]);
    if (lawId === scenarioLaw.id) {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([{ filename: scenarioLaw.scenarioFilename }]),
      });
    }
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: '[]',
    });
  });

  // GET /api/corpus/laws/{law_id}/scenarios/{filename} — return the
  // scenario text for any law that asks. We only have one fixture so we
  // serve it for every match; tests that need multiple scenario files can
  // refine this later.
  await page.route('**/api/corpus/laws/*/scenarios/*', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(/\/api\/corpus\/laws\/([^/]+)\/scenarios\/([^/]+)$/);
    if (!match) return route.fallback();
    return route.fulfill({
      status: 200,
      contentType: 'text/plain; charset=utf-8',
      body: scenarioFile,
    });
  });

  // /api/sources — corpus source list (used by library page).
  await page.route('**/api/sources', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([
        { id: 'local', name: 'Local Test Corpus', source_type: 'local', priority: 1, law_count: corpus.size },
      ]),
    }),
  );

  // /auth/status — OIDC disabled in tests; useAuth expects this shape.
  await page.route('**/auth/status', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ authenticated: false, oidc_configured: false }),
    }),
  );
}

/**
 * Resolve a law's display name from its YAML content. Mirrors the backend's
 * `resolve_display_name` logic: literal name → return it; `#ref` → find the
 * matching action output value.
 */
function resolveDisplayName(yamlContent) {
  try {
    const doc = yaml.load(yamlContent);
    if (!doc) return null;
    const name = doc.name;
    if (typeof name === 'string' && !name.startsWith('#')) return name;
    if (typeof name === 'string' && name.startsWith('#')) {
      const ref = name.slice(1);
      for (const article of doc.articles || []) {
        for (const action of article.machine_readable?.execution?.actions || []) {
          if (action.output === ref && typeof action.value === 'string') {
            return action.value;
          }
        }
      }
    }
  } catch { /* ignore parse errors */ }
  return null;
}

/**
 * Collect all outputs declared across all articles in a law's YAML.
 * Returns `[{ name, output_type, article_number }]`, deduplicated by name.
 */
function collectOutputs(yamlContent) {
  try {
    const doc = yaml.load(yamlContent);
    if (!doc?.articles) return [];
    const seen = new Set();
    const results = [];
    for (const article of doc.articles) {
      const outputs = article.machine_readable?.execution?.output || [];
      for (const out of outputs) {
        if (out.name && !seen.has(out.name)) {
          seen.add(out.name);
          results.push({
            name: out.name,
            output_type: out.type || 'string',
            article_number: String(article.number || ''),
          });
        }
      }
    }
    results.sort((a, b) => a.name.localeCompare(b.name));
    return results;
  } catch { return []; }
}
