#!/usr/bin/env node
// Mechanical anti-AI-tell filter for regelrecht docs prose.
//
// Two severities:
//   error  — signatures with ~zero false positives. Exit 1. Fix before shipping.
//            em-dashes, banned LLM phrases, "Overall,"-style summary openers.
//   warn   — heuristic tells a human must judge (a regex cannot tell a real
//            "not X, but Y" contrast from an incidental "not ... but" in one
//            sentence). Reported, but exit stays 0 unless --strict.
//
// This is a linter, not a judge. Rhythm, rule-of-three, register, and whether a
// flagged contrast is genuinely a tell still need the SKILL.md checklist and a
// human eye. It exists to catch the mechanical stuff so the human read can spend
// its attention on cadence.
//
// Scope: markdown/MDX prose under docs/src/content and the bilingual landing
// copy in docs/src/lib/landing-content.ts. Code is not prose — fenced blocks,
// inline `code`, and (in .ts) everything outside string literals are stripped
// before matching, so `--no-verify`, `a - b`, and `foo—bar` in code never trip.
//
// Usage:
//   node check-prose-style.mjs [path ...]     # defaults to the docs prose set
//   node check-prose-style.mjs --strict       # warnings fail too (exit 1)
//   node check-prose-style.mjs --help
//
// Default paths resolve against the repo root inferred from this file's location
// (.claude/skills/docs-writing/ -> repo root), so it runs from anywhere.

import { readdirSync, statSync, readFileSync, existsSync } from 'node:fs';
import { join, relative, resolve, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = fileURLToPath(new URL('.', import.meta.url));
// .claude/skills/docs-writing/ -> repo root is three levels up.
const REPO_ROOT = resolve(HERE, '..', '..', '..');

const DEFAULT_TARGETS = [
  'docs/src/content',
  'docs/src/lib/landing-content.ts',
];

// ---------------------------------------------------------------------------
// Rules. `level` is 'error' or 'warn'. Regexes run over already-code-stripped
// text. Keep messages short; SKILL.md carries the "why".
//
// The banned lists deliberately DROP words that carry a real technical meaning
// in this repo (realm — Keycloak; landscape — "IT-landschap"; journey —
// "klantreis"). A linter that cries wolf on domain vocabulary gets muted.
// ---------------------------------------------------------------------------

const RULES = [
  {
    id: 'em-dash',
    level: 'error',
    // em-dash and en-dash used as punctuation. A hyphen-minus is fine.
    re: /[—–]/g,
    msg: 'em-dash / en-dash',
    hint: 'Rewrite with a comma, period, parentheses, or colon. Never an em-dash.',
  },
  {
    id: 'banned-en',
    level: 'error',
    re: /\b(it is worth noting|it should be noted|it is important to (?:note|emphasi[sz]e)|in the current landscape|in a world where|at the intersection of|delve|deep dive|dive deep|tapestry|groundbreaking|game-changer|paradigm shift|buckle up|let'?s dive in|here'?s the thing)\b/gi,
    msg: 'banned phrase (EN)',
    hint: 'Rewrite the sentence from scratch — swapping one word leaves the cadence.',
  },
  {
    id: 'banned-nl',
    level: 'error',
    re: /\b(het is belangrijk om op te merken|het is vermeldenswaardig|in het huidige landschap|in een wereld waarin|laten we eerlijk zijn|laat dat even bezinken|baanbrekend|game-changer)\b/gi,
    msg: 'verboden frase (NL)',
    hint: 'Herschrijf de zin; één woord vervangen laat de cadans staan.',
  },
  {
    id: 'summary-opener',
    level: 'error',
    // "Overall," / "In summary," / "Kortom," as a paragraph opener at line start
    // (allowing markdown list/quote markers and bold before it).
    re: /^[ \t>*+-]*(?:\*\*)?(Overall|In summary|In conclusion|Kortom|Al met al|Samengevat)(?:\*\*)?,/gim,
    msg: 'summary-restating opener',
    hint: 'Trust the reader; cut the recap or fold it into the last real point.',
  },
  {
    id: 'not-x-but-y',
    level: 'warn',
    // "not X, but Y" / "niet X, maar Y". High recall, low precision: a regex
    // cannot tell a dramatic contrast from an incidental one. Human decides.
    re: /\bnot\b[^.?!;:\n]{1,55}?,\s+but\b|\bniet\b[^.?!;:\n]{1,55}?,\s+maar\b/gi,
    msg: '"not X, but Y" / "niet X, maar Y" contrast (review)',
    hint: 'If it is a dramatic binary contrast, state the positive claim directly. If incidental, ignore.',
  },
  {
    id: 'not-x-not-y-but-z',
    level: 'warn',
    // The three-beat build-up Anne flagged by name. Rarer, so worth its own id.
    re: /\bnot\b[^.?!\n]{1,45}?,\s*not\b[^.?!\n]{1,45}?,\s*but\b|\bniet\b[^.?!\n]{1,45}?,\s*niet\b[^.?!\n]{1,45}?,\s*maar\b/gi,
    msg: '"not X, not Y, but Z" triple contrast (review)',
    hint: 'The three-beat build-up is a signature tell. Rewrite as a plain statement.',
  },
];

// ---------------------------------------------------------------------------
// Code stripping. Preserve line numbers so findings point at the real line.
// ---------------------------------------------------------------------------

// Replace matched spans with same-length whitespace (newlines kept) so byte
// offsets and line numbers are unchanged.
function blankOut(text, re) {
  return text.replace(re, (m) => m.replace(/[^\n]/g, ' '));
}

function stripMarkdownCode(text) {
  let t = text;
  // Fenced code blocks ``` ... ``` and ~~~ ... ~~~ (also handles ```lang).
  t = blankOut(t, /^([ \t]*)(```|~~~)[^\n]*\n[\s\S]*?^\1\2[^\n]*$/gm);
  // Any dangling/unclosed fence to end of file.
  t = blankOut(t, /^[ \t]*(```|~~~)[\s\S]*$/m);
  // Inline code `...` (single or double backtick).
  t = blankOut(t, /(`+)[^\n]*?\1/g);
  return t;
}

// For .ts/.js: only the inside of string literals is prose. Blank everything
// else so identifiers, operators, and JSDoc dashes never match. Handles ' " `.
function stripToStringLiterals(text) {
  const out = [];
  let i = 0;
  const n = text.length;
  let quote = null;
  while (i < n) {
    const c = text[i];
    if (quote) {
      if (c === '\\') {
        out.push(text[i], text[i + 1] ?? '');
        i += 2;
        continue;
      }
      if (c === quote) {
        out.push(' ');
        quote = null;
        i++;
        continue;
      }
      out.push(c === '\n' ? '\n' : c);
      i++;
      continue;
    }
    if (c === "'" || c === '"' || c === '`') {
      quote = c;
      out.push(' ');
      i++;
      continue;
    }
    out.push(c === '\n' ? '\n' : ' ');
    i++;
  }
  return out.join('');
}

function proseOf(file, raw) {
  const ext = extname(file);
  if (ext === '.ts' || ext === '.js' || ext === '.mjs' || ext === '.cjs') {
    return stripToStringLiterals(raw);
  }
  // Markdown/MDX: keep frontmatter in place (title/description are reader-facing
  // prose), just strip code. The banned lists rarely false-positive on YAML keys.
  return stripMarkdownCode(raw);
}

// ---------------------------------------------------------------------------
// Walk targets -> file list.
// ---------------------------------------------------------------------------

const PROSE_EXT = new Set(['.md', '.mdx', '.ts', '.js', '.mjs', '.cjs']);

function collect(target) {
  const abs = resolve(REPO_ROOT, target);
  if (!existsSync(abs)) {
    console.error(`path not found: ${target}`);
    process.exit(2);
  }
  if (statSync(abs).isFile()) return [abs];
  const files = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir)) {
      if (entry === 'node_modules' || entry.startsWith('.')) continue;
      const full = join(dir, entry);
      if (statSync(full).isDirectory()) walk(full);
      else if (PROSE_EXT.has(extname(entry))) files.push(full);
    }
  };
  walk(abs);
  return files;
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------

const argv = process.argv.slice(2);
if (argv.includes('--help') || argv.includes('-h')) {
  console.log(
    'Usage: node check-prose-style.mjs [--strict] [path ...]\n\n' +
      'Scans docs prose for anti-AI-tell violations.\n' +
      '  errors (em-dashes, banned phrases, summary openers) always fail (exit 1).\n' +
      '  warnings ("not X but Y" contrasts) are reported; --strict makes them fail too.\n\n' +
      'No paths => the default docs prose set:\n' +
      DEFAULT_TARGETS.map((t) => '  ' + t).join('\n') +
      '\n\nPass a single file to check just that file.',
  );
  process.exit(0);
}

const strict = argv.includes('--strict');
const targets = argv.filter((a) => !a.startsWith('--'));
const files = (targets.length ? targets : DEFAULT_TARGETS).flatMap(collect);

const findings = [];
for (const file of files) {
  const raw = readFileSync(file, 'utf8');
  const prose = proseOf(file, raw);
  const rel = relative(REPO_ROOT, file);
  const rawLines = raw.split('\n');
  const lineStart = [];
  { let o = 0; for (const ln of rawLines) { lineStart.push(o); o += ln.length + 1; } }
  const lineOf = (idx) => {
    let lo = 0, hi = lineStart.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (lineStart[mid] <= idx) lo = mid; else hi = mid - 1; }
    return lo + 1;
  };
  for (const rule of RULES) {
    rule.re.lastIndex = 0;
    let m;
    while ((m = rule.re.exec(prose)) !== null) {
      const line = lineOf(m.index);
      const excerpt = (rawLines[line - 1] ?? '').trim().slice(0, 100);
      findings.push({ rel, line, rule, excerpt });
      if (m.index === rule.re.lastIndex) rule.re.lastIndex++; // guard zero-width
    }
  }
}

const errors = findings.filter((f) => f.rule.level === 'error');
const warns = findings.filter((f) => f.rule.level === 'warn');

function report(list) {
  list.sort((a, b) => a.rel.localeCompare(b.rel) || a.line - b.line);
  let lastFile = '';
  for (const f of list) {
    if (f.rel !== lastFile) { console.error(`  ${f.rel}`); lastFile = f.rel; }
    console.error(`    ${f.line}: [${f.rule.id}] ${f.rule.msg}`);
    console.error(`         ${f.excerpt}`);
    console.error(`         → ${f.rule.hint}`);
  }
}

if (errors.length === 0 && warns.length === 0) {
  console.log(`Prose style check passed: ${files.length} file(s), no tells found.`);
  process.exit(0);
}

if (errors.length) {
  console.error(`Prose style ERRORS: ${errors.length} finding(s).\n`);
  report(errors);
  console.error('');
}
if (warns.length) {
  console.error(`Prose style WARNINGS (review, not auto-fail): ${warns.length} finding(s).\n`);
  report(warns);
  console.error('');
}

const fail = errors.length > 0 || (strict && warns.length > 0);
if (fail) {
  console.error('Each hit means rewrite the sentence, not swap one word. See SKILL.md.');
  process.exit(1);
}
console.error('No errors (warnings above are advisory). See SKILL.md for the human checklist.');
process.exit(0);
