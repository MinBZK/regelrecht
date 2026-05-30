// Assert every internal link in the built site resolves to a real page and
// (when it carries an #anchor) to an id that actually exists on that page.
//
// Relative links resolve against the slash-less route the page is served at
// (nginx try_files serves <route>/index.html with no redirect, so "./x" from
// /a/b targets /a/x). That makes "./sibling" links fragile, and a wrong
// absolute prefix (e.g. /docs/components/x vs /components/x) silently 404s in
// production. pa11y only loads the pages in .pa11yci, so it never catches a
// dangling link. This check covers that gap directly against dist/. Run after
// `astro build`; non-zero exit fails the gate.
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIST = fileURLToPath(new URL('../dist/', import.meta.url));

function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const s = statSync(p);
    if (s.isDirectory()) out.push(...walk(p));
    else if (name.endsWith('.html')) out.push(p);
  }
  return out;
}

function routeFor(file) {
  let r = '/' + relative(DIST, file).replace(/\\/g, '/');
  r = r.replace(/index\.html$/, '').replace(/\.html$/, '');
  if (r.length > 1 && r.endsWith('/')) r = r.slice(0, -1);
  return r === '' ? '/' : r;
}

function normalize(route) {
  if (route.length > 1 && route.endsWith('/')) route = route.slice(0, -1);
  return route === '' ? '/' : route;
}

// First pass: read each file once, record the route it serves and the set of
// element ids on it. Cache the HTML so the link scan below does not re-read.
const files = walk(DIST);
const pageIds = new Map(); // route (no trailing slash, no .html) -> Set<id>
const routes = new Set();
const htmlByFile = new Map();

for (const f of files) {
  const html = readFileSync(f, 'utf8');
  htmlByFile.set(f, html);
  const route = routeFor(f);
  routes.add(route);
  const ids = new Set();
  for (const m of html.matchAll(/\sid="([^"]+)"/g)) ids.add(m[1]);
  for (const m of html.matchAll(/\sname="([^"]+)"/g)) ids.add(m[1]);
  pageIds.set(route, ids);
}

// Second pass: scan every <a href> and verify it resolves.
const problems = [];
const externalHosts = new Set();

for (const f of files) {
  const html = htmlByFile.get(f);
  const fromRoute = routeFor(f);
  // Only matches double-quoted hrefs. Astro's built HTML always double-quotes
  // attributes, so single-quoted/unquoted hrefs do not occur here; if that ever
  // changes this regex would silently skip them.
  for (const m of html.matchAll(/<a\b[^>]*\shref="([^"]+)"/g)) {
    let href = m[1].trim();
    if (!href) continue;
    if (href.startsWith('mailto:') || href.startsWith('tel:')) continue;
    if (/^https?:\/\//i.test(href)) {
      try { externalHosts.add(new URL(href).host); } catch {}
      continue;
    }
    if (href.startsWith('//')) continue;
    if (href.startsWith('#')) {
      const anchor = decodeURIComponent(href.slice(1));
      const ids = pageIds.get(fromRoute) || new Set();
      if (anchor && !ids.has(anchor)) {
        problems.push(`${fromRoute}: missing anchor #${anchor} (same page)`);
      }
      continue;
    }
    // strip query
    let [path, anchor] = href.split('#');
    path = path.split('?')[0];
    // resolve relative
    let target;
    if (path.startsWith('/')) target = path;
    else {
      // The page is served at <route> with NO trailing slash (nginx try_files
      // serves <route>/index.html for the slash-less URL, no redirect). So a
      // browser resolves "./x" relative to the slash-less URL: it replaces the
      // last segment. Model exactly that: base is the route itself, not the
      // route + "/". This matches what a user navigating via the slash-less
      // nav links actually experiences.
      target = new URL(path, 'http://x' + fromRoute).pathname;
    }
    target = normalize(target);
    // ignore asset files
    if (/\.(png|jpe?g|svg|webp|gif|ico|pdf|css|js|json|xml|txt|woff2?|zip|yaml|yml)$/i.test(target)) {
      if (!existsSync(join(DIST, target.replace(/^\//, '')))) {
        problems.push(`${fromRoute}: missing asset ${target}`);
      }
      continue;
    }
    if (!routes.has(target)) {
      problems.push(`${fromRoute}: broken link -> ${href} (resolved ${target})`);
      continue;
    }
    if (anchor) {
      const a = decodeURIComponent(anchor);
      const ids = pageIds.get(target) || new Set();
      if (!ids.has(a)) {
        problems.push(`${fromRoute}: missing anchor -> ${href} (#${a} not on ${target})`);
      }
    }
  }
}

console.log(`Checked ${files.length} pages, ${routes.size} routes.`);
console.log(`External hosts referenced: ${[...externalHosts].sort().join(', ')}`);
console.log('');
if (problems.length === 0) {
  console.log('No internal link problems found.');
} else {
  console.log(`${problems.length} problem(s):`);
  for (const p of problems.sort()) console.log('  - ' + p);
  process.exitCode = 1;
}
