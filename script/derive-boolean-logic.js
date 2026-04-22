#!/usr/bin/env node
// Rendert per YAML een audit-Markdown met boolean-logic per MR-action.
// Usage: node script/derive-boolean-logic.js <yaml-file> [<output-md>]
//
// Elke action uit machine_readable.execution.actions wordt omgezet naar
// wiskundige/boolean notatie. legal_basis.explanation wordt als wettekst-
// quote voor de formule geplaatst. definitions en untranslatables worden
// bondig opgesomd.

const yaml = require('../frontend/node_modules/js-yaml');
const fs = require('fs');
const path = require('path');

const OP = {
  AND: '∧', OR: '∨', NOT: '¬',
  EQUALS: '=', NOT_EQUALS: '≠',
  GREATER_THAN: '>', LESS_THAN: '<',
  GREATER_OR_EQUAL: '≥', LESS_OR_EQUAL: '≤',
  ADD: '+', SUBTRACT: '−', MULTIPLY: '×', DIVIDE: '÷',
  MAX: 'MAX', MIN: 'MIN', IN: '∈'
};

function renderVal(v, prec = 0) {
  if (v === null || v === undefined) return '∅';
  if (typeof v === 'string') return v.startsWith('$') ? v.slice(1) : JSON.stringify(v);
  if (typeof v === 'number' || typeof v === 'boolean') return String(v);
  if (Array.isArray(v)) return v.map(x => renderVal(x, prec)).join(', ');
  if (typeof v !== 'object') return String(v);

  const op = v.operation;
  if (!op) return JSON.stringify(v);

  if (op === 'NOT') return `¬${renderVal(v.value, 3)}`;
  if (op === 'AND' || op === 'OR') {
    const sep = ` ${OP[op]} `;
    const parts = (v.conditions || v.values || []).map(c => renderVal(c, 2));
    const joined = parts.join(sep);
    return prec > 1 ? `(${joined})` : joined;
  }
  if (['EQUALS','NOT_EQUALS','GREATER_THAN','LESS_THAN','GREATER_OR_EQUAL','LESS_OR_EQUAL'].includes(op)) {
    const s = renderVal(v.subject, 3);
    const val = renderVal(v.value, 3);
    // EQUALS x true => x,  EQUALS x false => ¬x
    if (op === 'EQUALS' && val === 'true') return s;
    if (op === 'EQUALS' && val === 'false') return `¬${s}`;
    return `${s} ${OP[op]} ${val}`;
  }
  if (['ADD','SUBTRACT','MULTIPLY','DIVIDE'].includes(op)) {
    const parts = (v.values || []).map(x => renderVal(x, 3));
    const joined = parts.join(` ${OP[op]} `);
    return prec > 2 ? `(${joined})` : joined;
  }
  if (op === 'MAX' || op === 'MIN') {
    const parts = (v.values || []).map(x => renderVal(x, 0));
    return `${OP[op]}(${parts.join(', ')})`;
  }
  if (op === 'IF') {
    const cases = (v.cases || []).map(c =>
      `IF ${renderVal(c.when, 0)}\n         THEN ${renderVal(c.then, 0)}`
    );
    const def = v.default !== undefined ? `\n         ELSE ${renderVal(v.default, 0)}` : '';
    return cases.join('\n         ELSE ') + def;
  }
  if (op === 'IN') {
    return `${renderVal(v.subject, 3)} ∈ [${renderVal(v.value, 0)}]`;
  }
  return JSON.stringify(v);
}

function renderAction(action) {
  const out = action.output;
  if (action.value !== undefined) {
    if (typeof action.value === 'boolean' || typeof action.value === 'number' || typeof action.value === 'string' && !action.value.startsWith('$')) {
      return `${out}  ≡  ${renderVal(action.value)}  *(constant)*`;
    }
    return `${out}  ≡\n    ${renderVal(action.value).replace(/\n/g, '\n    ')}`;
  }
  if (action.operation) {
    const v = { operation: action.operation, values: action.values, conditions: action.conditions, subject: action.subject };
    return `${out}  ≡\n    ${renderVal(v).replace(/\n/g, '\n    ')}`;
  }
  return `${out}  ≡  ${JSON.stringify(action)}`;
}

function main() {
  const args = process.argv.slice(2);
  if (!args[0]) {
    console.error('Usage: node script/derive-boolean-logic.js <yaml-file> [<output-md>]');
    process.exit(1);
  }
  const inPath = args[0];
  const d = yaml.load(fs.readFileSync(inPath, 'utf8'));

  const lines = [];
  lines.push(`# Boolean-logic derivatie — ${d.name || d['officiele_titel'] || d.$id}`);
  lines.push('');
  lines.push('**`$id`**: `' + d.$id + '`   **Wet-URL**: ' + (d.url || '-') + '   **Valid from**: ' + (d.valid_from || '-'));
  lines.push('');
  lines.push(`*Gegenereerd uit \`${path.relative(process.cwd(), inPath)}\`. Regenereer met \`just audit-boolean\` of \`node script/derive-boolean-logic.js <yaml>\`. Wijzig niet handmatig — edit de YAML en herschrijf deze file.*`);
  lines.push('');

  if (Array.isArray(d.legal_basis) && d.legal_basis.length) {
    lines.push('## Grondslagen (top-level `legal_basis`)');
    lines.push('');
    for (const lb of d.legal_basis) {
      lines.push(`- ${lb.law_id || lb.law} art ${lb.article}${lb.paragraph ? ' ' + lb.paragraph : ''}`);
    }
    lines.push('');
  }

  for (const art of (d.articles || [])) {
    const mr = art.machine_readable;
    if (!mr) continue;
    const acts = mr.execution?.actions || [];
    const defs = mr.definitions || {};
    const unts = mr.untranslatables || [];
    if (!acts.length && !Object.keys(defs).length && !unts.length) continue;

    lines.push(`## Artikel ${art.number}`);
    lines.push('');
    if (art.text) {
      const firstLine = art.text.split('\n').find(l => l.trim().startsWith('**')) || art.text.split('\n')[0];
      lines.push(`*${firstLine.trim()}*`);
      lines.push('');
    }
    if (art.url) {
      lines.push(`[wetbron](${art.url})`);
      lines.push('');
    }

    const inputs = mr.execution?.input || [];
    if (inputs.length) {
      lines.push('### Sources (input van andere wetten)');
      lines.push('');
      for (const i of inputs) {
        const s = i.source || {};
        lines.push(`- \`${i.name}\` ← **${s.regulation || '?'}** / \`${s.output || '?'}\``);
      }
      lines.push('');
    }

    if (Object.keys(defs).length) {
      lines.push('### Definities (constanten uit wettekst)');
      lines.push('');
      for (const [k, v] of Object.entries(defs)) {
        const unit = v.type_spec?.unit ? ` ${v.type_spec.unit}` : '';
        lines.push(`- \`${k}\` = **${v.value}**${unit} — ${v.description || ''}`);
      }
      lines.push('');
    }

    if (acts.length) {
      lines.push('### Formules');
      lines.push('');
      for (const a of acts) {
        const lb = a.legal_basis;
        if (lb?.explanation) {
          lines.push(`> **Wettekst-attributie** (${lb.law || d.$id} art ${lb.article || '?'}${lb.paragraph ? ' ' + lb.paragraph : ''}): ${lb.explanation}`);
          lines.push('');
        }
        lines.push('```');
        lines.push(renderAction(a));
        lines.push('```');
        lines.push('');
        lines.push(`- [ ] Formule klopt met wettekst`);
        lines.push(`- [ ] Attributie verwijst naar juiste bron-artikel`);
        lines.push('');
      }
    }

    if (unts.length) {
      lines.push('### Untranslatables (bewust niet in formule)');
      lines.push('');
      for (const u of unts) {
        lines.push(`- **${u.construct}**${u.accepted ? ' *(geaccepteerd)*' : ''}`);
        lines.push(`  ${u.reason}`);
        lines.push(`  - [ ] Reden accepteerbaar`);
      }
      lines.push('');
    }
  }

  const outPath = args[1] || inPath.replace(/\.yaml$/, '.formulas.md').replace(/(^|\/)corpus\/regulation\//, '$1docs/audit/corpus/');
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, lines.join('\n'));
  console.log(`written: ${outPath} (${lines.length} lines)`);
}

main();
