import { cpSync, mkdirSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const src = resolve(root, '..', 'corpus', 'central', 'regulation', 'nl', 'wet', 'wet_op_de_zorgtoeslag', '2025-01-01.yaml');
const dest = resolve(root, 'public', 'data', 'zorgtoeslagwet-2025-01-01.yaml');

mkdirSync(dirname(dest), { recursive: true });
cpSync(src, dest);
console.log('Copied regulation YAML to public/data/');
