/**
 * Copy regulation YAML files to public/data/ based on corpus-registry.yaml.
 *
 * Processes LOCAL sources only: copies their YAML files and generates
 * index.json with metadata. GitHub sources are resolved at runtime
 * in the browser via the GitHub Tree API.
 *
 * Also copies corpus-registry.yaml to public/ so the browser can read
 * GitHub source configuration at runtime.
 */
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import { resolve, dirname, relative } from 'path';
import { fileURLToPath } from 'url';
import yaml from 'js-yaml';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const projectRoot = resolve(root, '..');
const destDir = resolve(root, 'public', 'data');

const registryPath = resolve(projectRoot, process.env.CORPUS_REGISTRY_PATH || 'corpus-registry.yaml');
const localOverridePath = resolve(projectRoot, process.env.CORPUS_REGISTRY_LOCAL_PATH || 'corpus-registry.local.yaml');

/** Load and merge registry manifest with optional local override. */
function loadRegistry() {
  if (!existsSync(registryPath)) {
    console.warn(`Registry not found: ${registryPath}`);
    return { sources: [] };
  }

  const base = yaml.load(readFileSync(registryPath, 'utf-8'));

  if (existsSync(localOverridePath)) {
    const override = yaml.load(readFileSync(localOverridePath, 'utf-8'));
    if (override?.sources) {
      const overrideIds = new Set(override.sources.map(s => s.id));
      base.sources = base.sources.filter(s => !overrideIds.has(s.id)).concat(override.sources);
      console.log(`Merged ${override.sources.length} source(s) from local override`);
    }
  }

  base.sources.sort((a, b) => (a.priority || 0) - (b.priority || 0));
  return base;
}

/** Recursively find all .yaml files under a directory. */
function findYamlFiles(dir) {
  if (!existsSync(dir)) return [];
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = resolve(dir, entry);
    if (statSync(full).isDirectory()) {
      results.push(...findYamlFiles(full));
    } else if (entry.endsWith('.yaml')) {
      results.push(full);
    }
  }
  return results;
}

/** Recursively find all .feature files under a directory. */
function findFeatureFiles(dir) {
  if (!existsSync(dir)) return [];
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = resolve(dir, entry);
    if (statSync(full).isDirectory()) {
      results.push(...findFeatureFiles(full));
    } else if (entry.endsWith('.feature')) {
      results.push(full);
    }
  }
  return results;
}

/** Extract metadata from a corpus YAML.
 *
 * Uses js-yaml so block scalars (`>-`, `|`) and quoted strings are decoded
 * correctly — the previous line-based parser captured the literal `>-`
 * marker as the value, which broke the library entry for any YAML that
 * folded its name across multiple lines.
 */
function extractMeta(content) {
  try {
    const doc = yaml.load(content);
    if (!doc || typeof doc !== 'object') return {};
    const trim = (v) => (typeof v === 'string' ? v.trim().replace(/\s+/g, ' ') : v);
    return {
      id: trim(doc.$id),
      regulatory_layer: trim(doc.regulatory_layer),
      publication_date: trim(doc.publication_date),
      name: trim(doc.name),
      officiele_titel: trim(doc.officiele_titel),
    };
  } catch {
    return {};
  }
}

// --- Main ---

mkdirSync(destDir, { recursive: true });

const registry = loadRegistry();
const localSources = registry.sources.filter(s => s.type === 'local');
const githubSources = registry.sources.filter(s => s.type === 'github');

// Copy registry to public/ so the browser can read GitHub source config at runtime.
if (existsSync(registryPath)) {
  cpSync(registryPath, resolve(root, 'public', 'corpus-registry.yaml'));
}

const multiSource = registry.sources.length > 1;
const index = [];
let totalFiles = 0;

for (const source of localSources) {
  const sourceDir = resolve(projectRoot, source.local.path);
  const yamlFiles = findYamlFiles(sourceDir);
  console.log(`Source "${source.name}" (${source.id}, priority ${source.priority}): ${yamlFiles.length} files from ${source.local.path}`);

  // Deduplicate: keep latest publication_date per $id.
  const latestById = new Map();
  const parsed = [];
  for (const filePath of yamlFiles) {
    const content = readFileSync(filePath, 'utf-8');
    const meta = extractMeta(content);
    if (!meta.id) continue;
    const relPath = relative(sourceDir, filePath);
    parsed.push({ relPath, content, meta });
    const existing = latestById.get(meta.id);
    if (!existing || (meta.publication_date || '') > (existing.meta.publication_date || '')) {
      latestById.set(meta.id, { relPath, meta });
    }
  }

  console.log(`  ${yamlFiles.length} files → ${latestById.size} unique laws (${parsed.length} versions on disk)`);

  // Write ALL versions to disk.
  for (const { relPath, content } of parsed) {
    const destRel = multiSource ? `${source.id}/${relPath}` : relPath;
    const dest = resolve(destDir, destRel);
    mkdirSync(dirname(dest), { recursive: true });
    writeFileSync(dest, content);
    totalFiles++;
  }

  // Copy scenario .feature files alongside laws.
  const featureFiles = findFeatureFiles(sourceDir);
  for (const filePath of featureFiles) {
    const relPath = relative(sourceDir, filePath);
    const destRel = multiSource ? `${source.id}/${relPath}` : relPath;
    const dest = resolve(destDir, destRel);
    mkdirSync(dirname(dest), { recursive: true });
    cpSync(filePath, dest);
  }
  if (featureFiles.length > 0) {
    console.log(`  ${featureFiles.length} scenario files copied`);
  }

  // Add latest version per $id to the index.
  for (const [, { relPath, meta }] of latestById) {
    const destRel = multiSource ? `${source.id}/${relPath}` : relPath;
    index.push({
      id: meta.id,
      name: meta.name || meta.officiele_titel || meta.id,
      regulatory_layer: meta.regulatory_layer || 'unknown',
      publication_date: meta.publication_date || 'unknown',
      path: `/data/${destRel}`,
      source_id: source.id,
      source_name: source.name,
    });
  }
}

index.sort((a, b) =>
  a.regulatory_layer.localeCompare(b.regulatory_layer) || a.id.localeCompare(b.id)
);

writeFileSync(resolve(destDir, 'index.json'), JSON.stringify(index, null, 2));
console.log(`Done: ${totalFiles} files, ${index.length} laws in index`);
if (githubSources.length > 0) {
  console.log(`${githubSources.length} GitHub source(s) will be resolved at runtime in the browser.`);
}
