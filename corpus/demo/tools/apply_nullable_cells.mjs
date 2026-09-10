#!/usr/bin/env node
/**
 * Bring the `null` cells of the demo scenario data tables in line with the
 * `nullable` declarations of the laws (RFC-036, schema v0.5.8).
 *
 * A `null` cell in a register table is an explicit absence handed to the
 * engine. Since nullability is a property of the type, the engine refuses a
 * `null` for an input the law declares as never absent ("declares as never
 * absent"), so a `null` cell is only valid in a column whose input carries
 * `nullable: true`. For every other column with a `null` cell, this tool
 * writes what the binding says a missing row means:
 *
 *  - `absent: 0` (an income or count register): the cell becomes `0`, the
 *    register counts nothing there;
 *  - anything else (`absent: unknown`, no `absent`, a claim, an array): the
 *    cell becomes empty, the key is omitted and the input is unknown.
 *
 * A `null` in the key column is left alone (it never occurs). Parameter steps
 * (`parameter "x" is "null"`) are not touched: declare_nullable.py marks every
 * parameter a scenario passes as null nullable, so those steps stay valid.
 *
 * Idempotent and text-based, like apply_absent_semantics.mjs; every rewritten
 * file gets a header comment naming this tool. Run from the repository root:
 *
 *     node corpus/demo/tools/apply_nullable_cells.mjs corpus/demo
 */

import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..', '..', '..');
const yaml = createRequire(join(root, 'frontend', 'package.json'))('js-yaml');

const HEADER_MARK = '# Null cells aligned with the nullable declarations by corpus/demo/tools/apply_nullable_cells.mjs (RFC-036)';
const DATA_STEP = /^(\s*)(Given|And|But|\*) the following "([^"]+)" data with key "([^"]+)" for law "([^"]+)":\s*$/;

function walk(dir, predicate, out = []) {
  for (const entry of readdirSync(dir).sort()) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, predicate, out);
    else if (predicate(entry)) out.push(full);
  }
  return out;
}

/** Per law id, per input: { type, nullable } (nullable in any version counts). */
function inputDeclarations(nlDir) {
  const inputs = {};
  for (const file of walk(nlDir, (n) => n.endsWith('.yaml'))) {
    const doc = yaml.load(readFileSync(file, 'utf8'));
    if (!doc?.$id) continue;
    for (const article of doc.articles ?? []) {
      for (const input of article.machine_readable?.execution?.input ?? []) {
        const prev = (inputs[doc.$id] ??= {})[input.name];
        inputs[doc.$id][input.name] = { type: input.type ?? 'string', nullable: Boolean(prev?.nullable) || input.nullable === true };
      }
    }
  }
  return inputs;
}

function splitRow(line) {
  const cells = [];
  let cur = '';
  const s = line.trim();
  for (let i = 1; i < s.length; i += 1) {
    const c = s[i];
    if (c === '\\' && s[i + 1] === '|') {
      cur += '\\|';
      i += 1;
    } else if (c === '|') {
      cells.push(cur.trim());
      cur = '';
    } else {
      cur += c;
    }
  }
  return cells;
}

function renderTable(rows, indent) {
  const widths = [];
  for (const row of rows) row.forEach((c, i) => { widths[i] = Math.max(widths[i] ?? 0, c.length); });
  return rows.map((row) => `${indent}| ${row.map((c, i) => c.padEnd(widths[i])).join(' | ')} |`);
}

function rewriteTable(tableLines, law, keyField, bindings, inputs, counts, where) {
  const rows = tableLines.map(splitRow);
  const [header, ...body] = rows;
  let changed = false;
  for (const row of body) {
    header.forEach((column, i) => {
      if (column === keyField || row[i] !== 'null') return;
      const declaration = inputs[law]?.[column];
      if (declaration?.nullable) return;
      const binding = bindings[law]?.[column];
      const absent = binding && binding.kind === 'table' && Object.hasOwn(binding, 'absent') ? binding.absent : 'unknown';
      if (absent === 0 && declaration?.type !== 'array') {
        row[i] = '0';
        counts.nullToZero += 1;
      } else {
        row[i] = '';
        counts.nullToEmpty += 1;
      }
      (counts.columns[`${law}.${column}`] ??= { to: row[i] === '0' ? '0' : 'empty', cells: 0, files: new Set() });
      counts.columns[`${law}.${column}`].cells += 1;
      counts.columns[`${law}.${column}`].files.add(where);
      changed = true;
    });
  }
  if (!changed) return null;
  const indent = /^\s*/.exec(tableLines[0])[0];
  return renderTable([header, ...body], indent);
}

function rewriteFeature(text, bindings, inputs, counts, where) {
  const lines = text.split('\n');
  const out = [];
  let changed = false;
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const step = DATA_STEP.exec(line);
    if (!step) {
      out.push(line);
      i += 1;
      continue;
    }
    const tableLines = [];
    let j = i + 1;
    while (j < lines.length && /^\s*\|/.test(lines[j])) {
      tableLines.push(lines[j]);
      j += 1;
    }
    const [, , , , keyField, law] = step;
    const replacement = rewriteTable(tableLines, law, keyField, bindings, inputs, counts, where);
    out.push(line, ...(replacement ?? tableLines));
    if (replacement) changed = true;
    i = j;
  }
  if (!changed) return null;
  if (!out.includes(HEADER_MARK)) {
    // After the other tool headers, before the Feature line.
    let at = 0;
    while (at < out.length && out[at].startsWith('#')) at += 1;
    out.splice(at, 0, HEADER_MARK);
  }
  return out.join('\n');
}

function main() {
  const demoDir = resolve(process.argv[2] ?? 'corpus/demo');
  const nlDir = join(demoDir, 'regulation', 'nl');
  const bindings = yaml.load(readFileSync(join(demoDir, 'bindings.yaml'), 'utf8'));
  const inputs = inputDeclarations(nlDir);
  const counts = { files: 0, changed: 0, nullToZero: 0, nullToEmpty: 0, columns: {} };
  for (const file of walk(nlDir, (n) => n.endsWith('.feature'))) {
    counts.files += 1;
    const where = relative(nlDir, file);
    const text = readFileSync(file, 'utf8');
    const rewritten = rewriteFeature(text, bindings, inputs, counts, where);
    if (rewritten !== null && rewritten !== text) {
      writeFileSync(file, rewritten);
      counts.changed += 1;
    }
  }
  for (const [column, c] of Object.entries(counts.columns).sort()) {
    console.log(`${column}: ${c.cells} null cell(s) -> ${c.to} in ${c.files.size} file(s)`);
  }
  console.log(`${counts.files} feature files, ${counts.changed} rewritten: ${counts.nullToZero} null -> 0, ${counts.nullToEmpty} null -> empty`);
}

main();
