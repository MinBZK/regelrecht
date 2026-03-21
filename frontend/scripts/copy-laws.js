import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import { resolve, dirname, relative } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const corpusDir = resolve(root, '..', 'corpus', 'regulation', 'nl');
const destDir = resolve(root, 'public', 'data');

if (!existsSync(corpusDir)) {
  console.warn(`Corpus directory not found: ${corpusDir}`);
  console.warn('Skipping law copy — library will show no laws.');
  mkdirSync(destDir, { recursive: true });
  writeFileSync(resolve(destDir, 'index.json'), '[]');
  process.exit(0);
}

/** Recursively find all .yaml files under a directory. */
function findYamlFiles(dir, base = dir) {
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = resolve(dir, entry);
    if (statSync(full).isDirectory()) {
      results.push(...findYamlFiles(full, base));
    } else if (entry.endsWith('.yaml')) {
      results.push(full);
    }
  }
  return results;
}

/** Extract $id and other metadata from YAML using line-based parsing. */
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

mkdirSync(destDir, { recursive: true });

const yamlFiles = findYamlFiles(corpusDir);
const index = [];

for (const src of yamlFiles) {
  const rel = relative(corpusDir, src);
  const dest = resolve(destDir, rel);

  mkdirSync(dirname(dest), { recursive: true });
  cpSync(src, dest);

  const content = readFileSync(src, 'utf-8');
  const meta = extractMeta(content);

  if (meta.id) {
    index.push({
      id: meta.id,
      name: meta.name || meta.officiele_titel || meta.id,
      regulatory_layer: meta.regulatory_layer || 'unknown',
      publication_date: meta.publication_date || 'unknown',
      path: `/data/${rel}`,
    });
  }
}

index.sort((a, b) =>
  a.regulatory_layer.localeCompare(b.regulatory_layer) || a.id.localeCompare(b.id)
);

writeFileSync(resolve(destDir, 'index.json'), JSON.stringify(index, null, 2));
console.log(`Copied ${yamlFiles.length} regulation files, generated index.json with ${index.length} laws`);
