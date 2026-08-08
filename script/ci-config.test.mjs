// Pint drie lijsten vast die alleen fout kunnen gaan door stil weg te driften
// van de rest van de repo. Alle drie falen zonder klacht: de nachtelijke
// opruiming slaagt ook als hij een image overslaat, en een Dockerfile zonder
// dependabot-blok blijft gewoon bouwen op een base die niemand meer bijwerkt.
//
// Geen yaml-parser: node heeft er geen, en deze drie velden staan in
// voorspelbare, platte vorm in de workflows.

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const read = (p) => readFileSync(join(ROOT, p), 'utf8');

const deploy = read('.github/workflows/deploy.yml');
const cleanup = read('.github/workflows/scheduled-cleanup.yml');
const dependabot = read('.github/dependabot.yml');

const builtImages = [...deploy.matchAll(/^\s*image-name:\s*\S+\/(\S+)\s*$/gm)].map((m) => m[1]);

const trackedDockerfiles = spawnSync('git', ['ls-files', '--full-name'], { cwd: ROOT, encoding: 'utf8' })
  .stdout.split('\n')
  .filter((p) => basename(p) === 'Dockerfile');

const dependabotDockerDirs = new Set(
  [...dependabot.matchAll(/package-ecosystem:\s*docker\n\s*directory:\s*(\S+)/g)].map((m) => m[1]),
);

test('deploy.yml bouwt images en de lijst is niet leeg', () => {
  assert.ok(builtImages.length >= 8, `slechts ${builtImages.length} image-name-regels gevonden`);
});

test('de nachtelijke opruiming kent elk gebouwd image', () => {
  const loop = cleanup.match(/^\s*for PACKAGE in (.+); do$/m);
  assert.ok(loop, 'geen `for PACKAGE in …` gevonden in scheduled-cleanup.yml');
  const cleaned = new Set(loop[1].trim().split(/\s+/));

  for (const image of builtImages) {
    assert.ok(cleaned.has(image), `${image} wordt gebouwd maar nooit opgeruimd`);
  }
  for (const image of cleaned) {
    assert.ok(builtImages.includes(image), `${image} wordt opgeruimd maar nergens gebouwd`);
  }
});

test('elke Dockerfile in de boom heeft een dependabot-docker-blok', () => {
  assert.ok(trackedDockerfiles.length >= 6, `slechts ${trackedDockerfiles.length} Dockerfiles gevonden`);

  for (const file of trackedDockerfiles) {
    const dir = `/${dirname(file)}`.replace(/^\/\.$/, '/');
    assert.ok(dependabotDockerDirs.has(dir), `${file} heeft geen dependabot-blok voor ${dir}`);
  }
});

test('elke dependabot-docker-directory wijst op een echte Dockerfile', () => {
  const dirs = new Set(trackedDockerfiles.map((f) => `/${dirname(f)}`.replace(/^\/\.$/, '/')));
  for (const dir of dependabotDockerDirs) {
    assert.ok(dirs.has(dir), `dependabot bewaakt ${dir}, maar daar staat geen Dockerfile`);
  }
});

test('een node-ignore staat alleen bij een Dockerfile met een node-stage', () => {
  // Splits op de blokken; `directory:` staat direct onder `package-ecosystem:`.
  const blocks = dependabot.split(/(?=^\s*-\s*package-ecosystem:)/m);
  for (const block of blocks) {
    const dir = block.match(/directory:\s*(\S+)/)?.[1];
    if (!dir || !/package-ecosystem:\s*docker/.test(block)) continue;
    if (!/dependency-name:\s*node\b/.test(block)) continue;

    const dockerfile = join(ROOT, dir === '/' ? '' : dir.slice(1), 'Dockerfile');
    const content = readFileSync(dockerfile, 'utf8');
    assert.match(content, /^FROM\s+node:/m, `${dir} negeert node-versies maar heeft geen node-stage`);
  }
});
