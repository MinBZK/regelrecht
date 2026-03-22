/**
 * Copy regulation YAML files to public/data/ based on corpus-registry.yaml.
 *
 * Reads the registry manifest (and optional local override), iterates all
 * local sources, copies their YAML files, and generates an index.json with
 * metadata and source provenance.
 *
 * This is the same source discovery mechanism the Rust engine uses at runtime,
 * ensuring the editor sees the same laws as the engine.
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
      // Local entries replace base entries with same id (full replacement)
      const overrideIds = new Set(override.sources.map(s => s.id));
      base.sources = base.sources.filter(s => !overrideIds.has(s.id)).concat(override.sources);
      console.log(`Merged ${override.sources.length} source(s) from local override`);
    }
  }

  // Sort by priority (lower = higher priority)
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

/** Extract metadata from YAML using line-based parsing (no full parse). */
function extractMeta(content) {
  const meta = {};
  for (const line of content.split('\n')) {
    if (line.startsWith('$id:')) {
      meta.id = line.slice(4).trim().replace(/^['"]|['"]$/g, '');
    } else if (line.startsWith('regulatory_layer:')) {
      meta.regulatory_layer = line.slice(17).trim().replace(/^['"]|['"]$/g, '');
    } else if (line.startsWith('publication_date:')) {
      meta.publication_date = line.slice(17).trim().replace(/^['"]|['"]$/g, '');
    } else if (line.startsWith('name:')) {
      meta.name = line.slice(5).trim().replace(/^['"]|['"]$/g, '');
    } else if (line.startsWith('officiele_titel:')) {
      meta.officiele_titel = line.slice(16).trim().replace(/^['"]|['"]$/g, '');
    }
  }
  return meta;
}

// --- Main ---

mkdirSync(destDir, { recursive: true });

const registry = loadRegistry();
const localSources = registry.sources.filter(s => s.type === 'local');

if (localSources.length === 0) {
  console.warn('No local sources in registry — library will show no laws.');
  writeFileSync(resolve(destDir, 'index.json'), '[]');
  process.exit(0);
}

const index = [];
const seenIds = new Map(); // $id → { priority, source_id } (for cross-source conflict resolution)
let totalFiles = 0;

for (const source of localSources) {
  const sourceDir = resolve(projectRoot, source.local.path);
  const yamlFiles = findYamlFiles(sourceDir);

  console.log(`Source "${source.name}" (${source.id}, priority ${source.priority}): ${yamlFiles.length} files from ${source.local.path}`);

  for (const src of yamlFiles) {
    const content = readFileSync(src, 'utf-8');
    const meta = extractMeta(content);

    if (!meta.id) continue;

    const rel = relative(sourceDir, src);
    // Namespace destination by source id to avoid cross-source file collisions
    const destRel = localSources.length > 1 ? `${source.id}/${rel}` : rel;
    const dest = resolve(destDir, destRel);

    mkdirSync(dirname(dest), { recursive: true });
    cpSync(src, dest);
    totalFiles++;

    // Cross-source conflict resolution (same as Rust SourceMap).
    // Multiple versions within the same source are allowed (temporal versioning).
    const existing = seenIds.get(meta.id);
    if (existing !== undefined && existing.source_id !== source.id) {
      if (source.priority === existing.priority) {
        console.error(`Conflict: law '${meta.id}' provided by sources '${existing.source_id}' and '${source.id}' with equal priority ${source.priority}`);
        process.exit(1);
      }
      if (source.priority > existing.priority) continue; // existing wins (lower = higher priority)
      // New source wins — remove old entries from losing source
      const loserId = existing.source_id;
      for (let i = index.length - 1; i >= 0; i--) {
        if (index[i].id === meta.id && index[i].source_id === loserId) index.splice(i, 1);
      }
    }
    seenIds.set(meta.id, { priority: source.priority, source_id: source.id });

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
console.log(`Done: ${totalFiles} files from ${localSources.length} source(s), ${index.length} laws in index`);
