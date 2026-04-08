/**
 * Parse Gherkin .feature files into JSON for the graph visualization.
 *
 * Usage: node scripts/generate-features-data.js [features-dir]
 * Default features dir: ../../features/
 * Output: public/data/features-data.json
 */
import { readdirSync, readFileSync, mkdirSync, writeFileSync } from 'fs';
import { resolve, dirname, basename } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const featuresDir = resolve(__dirname, process.argv[2] || '../../features');
// Write to both public/data (for Vite) and data/ (for static server)
const outDirPublic = resolve(__dirname, '..', 'public', 'data');
const outDir = resolve(__dirname, '..', 'data');

function parseTable(lines, startIdx) {
  const rows = [];
  let i = startIdx;
  while (i < lines.length && lines[i].trim().startsWith('|')) {
    const cells = lines[i].trim().split('|').filter(c => c !== '').map(c => c.trim());
    rows.push(cells);
    i++;
  }
  if (rows.length < 2) return { table: rows.length === 1 ? rows[0].map(c => [c]) : [], endIdx: i };

  // First row is header, rest are data rows → convert to [key, value] pairs
  const headers = rows[0];
  if (rows.length === 2) {
    // Single data row: zip headers with values
    const values = rows[1];
    const table = headers.map((h, idx) => [h, values[idx] || '']);
    return { table, endIdx: i };
  }
  // Multiple data rows: return as-is (header + rows)
  return { table: rows, endIdx: i };
}

function parseFeatureFile(filePath) {
  const content = readFileSync(filePath, 'utf-8');
  const lines = content.split('\n');
  const filename = basename(filePath);

  const feature = {
    name: '',
    filename,
    scenarios: [],
  };

  let currentScenario = null;
  let backgroundSteps = [];
  let inBackground = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();

    // Skip empty lines and comments
    if (!line || line.startsWith('#')) continue;

    if (line.startsWith('Feature:')) {
      feature.name = line.slice('Feature:'.length).trim();
      continue;
    }

    if (line.startsWith('Background:')) {
      inBackground = true;
      continue;
    }

    if (line.startsWith('Scenario:') || line.startsWith('Scenario Outline:')) {
      inBackground = false;
      if (currentScenario) {
        feature.scenarios.push(currentScenario);
      }
      const isOutline = line.startsWith('Scenario Outline:');
      const name = line.slice(isOutline ? 'Scenario Outline:'.length : 'Scenario:'.length).trim();
      currentScenario = {
        type: isOutline ? 'scenario_outline' : 'scenario',
        name,
        steps: [...backgroundSteps],
      };
      continue;
    }

    // Step keywords
    const stepMatch = line.match(/^(Given|When|Then|And|But)\s+(.+)/);
    if (stepMatch) {
      const keyword = stepMatch[1];
      const text = stepMatch[2];
      // Normalize "And"/"But" to inherit the previous keyword's semantic
      const step = { keyword, text };

      // Check if next line starts a table
      if (i + 1 < lines.length && lines[i + 1].trim().startsWith('|')) {
        const { table, endIdx } = parseTable(lines, i + 1);
        step.table = table;
        i = endIdx - 1;
      }

      if (inBackground) {
        backgroundSteps.push(step);
      } else if (currentScenario) {
        currentScenario.steps.push(step);
      }
    }
  }

  if (currentScenario) {
    feature.scenarios.push(currentScenario);
  }

  return feature;
}

// Main
const featureFiles = readdirSync(featuresDir).filter(f => f.endsWith('.feature'));
console.log(`Parsing ${featureFiles.length} feature files from ${featuresDir}`);

const features = featureFiles.map(f => parseFeatureFile(resolve(featuresDir, f)));

const totalScenarios = features.reduce((sum, f) => sum + f.scenarios.length, 0);
console.log(`Parsed ${totalScenarios} scenarios across ${features.length} features`);

const jsonContent = JSON.stringify(features, null, 2);
for (const dir of [outDir, outDirPublic]) {
  mkdirSync(dir, { recursive: true });
  const outPath = resolve(dir, 'features-data.json');
  writeFileSync(outPath, jsonContent);
  console.log(`Written to ${outPath}`);
}
