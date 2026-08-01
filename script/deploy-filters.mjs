// Bepaal welke deploybare componenten door een wijziging geraakt worden.
//
// Waarom niet met de hand: `deploy.yml` somde per component de paden op die
// hem raken, en die lijst dreef weg. `packages/auth` en `packages/law-model`
// stonden er in geen enkele filter, terwijl editor-api en admin op auth leunen
// en engine, corpus, pipeline en harvester op law-model. Een merge die alleen
// zo'n crate raakte bouwde dus geen image en rolde niets uit, zonder dat er
// iets rood werd. Dit script leidt de Rust-kant af uit de dependency-graaf van
// cargo, zodat een nieuwe crate vanzelf op de goede plek terechtkomt.
//
// De niet-Rust-kant blijft met de hand: frontends, Dockerfiles, het corpus en
// de skills die de enrich-worker meeneemt. Die verzameling is klein en
// verandert zelden.
//
// Faalt hier iets — cargo ontbreekt, de graaf is onleesbaar, een pad valt
// buiten alle regels — dan zet het script alles op `true`. De duurste uitkomst
// is dan een overbodige build; de goedkope uitkomst zou een stille overslag
// zijn, en dat is precies de fout die dit script moet uitbannen.

import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync } from 'node:fs';

// Per component: de crate waar het image aan hangt (of null), plus de paden
// buiten de graaf die het image beïnvloeden.
const COMPONENTS = {
  editor: {
    crate: 'regelrecht-editor-api',
    paths: [
      'frontend/',
      'packages/frontend-shared/',
      'corpus-registry.yaml',
      'corpus/regulation/',
    ],
  },
  admin: {
    crate: 'regelrecht-admin',
    paths: ['packages/frontend-shared/', 'packages/admin/Dockerfile'],
  },
  'harvester-worker': {
    crate: 'regelrecht-pipeline',
    paths: ['packages/pipeline/Dockerfile'],
  },
  'enrich-worker': {
    crate: 'regelrecht-pipeline',
    paths: ['packages/pipeline/Dockerfile', '.claude/skills/law-'],
  },
  'pipeline-api': {
    crate: 'regelrecht-pipeline',
    paths: ['packages/pipeline/Dockerfile'],
  },
  grafana: { crate: null, paths: ['packages/grafana/'] },
  lawmaking: {
    crate: null,
    paths: ['frontend-lawmaking/', 'packages/frontend-shared/'],
  },
  docs: { crate: null, paths: ['docs/'] },
};

// Raakt elk component met een Rust-image: de workspace zelf.
const RUST_WIDE = [
  'packages/Cargo.lock',
  'packages/Cargo.toml',
  'packages/.cargo/',
  'rust-toolchain.toml',
];

function workspaceGraph() {
  const raw = execFileSync(
    'cargo',
    ['metadata', '--no-deps', '--format-version', '1'],
    { cwd: 'packages', maxBuffer: 64 * 1024 * 1024, encoding: 'utf8' },
  );
  const meta = JSON.parse(raw);
  const byName = new Map(meta.packages.map((p) => [p.name, p]));

  // Alleen `normal` en `build`: een dev-dependency zit niet in de binary, dus
  // een wijziging daarin hoeft geen image te bouwen.
  const closure = (name, seen = new Set()) => {
    for (const dep of byName.get(name)?.dependencies ?? []) {
      const kind = dep.kind ?? 'normal';
      if (!byName.has(dep.name) || seen.has(dep.name) || kind === 'dev') continue;
      seen.add(dep.name);
      closure(dep.name, seen);
    }
    return seen;
  };

  const dirOf = (name) => {
    const manifest = byName.get(name)?.manifest_path ?? '';
    const idx = manifest.lastIndexOf('/packages/');
    return idx === -1 ? null : manifest.slice(idx + 1).replace(/Cargo\.toml$/, '');
  };

  return { byName, closure, dirOf };
}

function main() {
  // Uit een bestand als het er is: een grote PR heeft meer bestandsnamen dan
  // er in een commandoregel passen.
  const args = process.argv.slice(2);
  const changed =
    args[0] === '--from-file'
      ? readFileSync(args[1], 'utf8').split('\n').filter(Boolean)
      : args.filter(Boolean);
  const out = process.env.GITHUB_OUTPUT;
  const emit = (name, value) => {
    console.log(`${name}=${value}`);
    if (out) appendFileSync(out, `${name}=${value}\n`);
  };

  if (changed.length === 0) {
    console.log('Geen lijst met gewijzigde bestanden: alles bouwen.');
    for (const name of Object.keys(COMPONENTS)) emit(name, true);
    return;
  }

  const { byName, closure, dirOf } = workspaceGraph();

  for (const [name, spec] of Object.entries(COMPONENTS)) {
    const prefixes = [...spec.paths];
    if (spec.crate) {
      if (!byName.has(spec.crate)) {
        throw new Error(`crate ${spec.crate} niet gevonden in de workspace`);
      }
      prefixes.push(...RUST_WIDE);
      for (const dep of [spec.crate, ...closure(spec.crate)]) {
        const dir = dirOf(dep);
        if (!dir) throw new Error(`geen pad voor crate ${dep}`);
        prefixes.push(dir);
      }
    }
    const hit = changed.some((file) => prefixes.some((p) => file.startsWith(p)));
    emit(name, hit);
  }
}

try {
  main();
} catch (error) {
  console.log(`::warning::deploy-filters faalde (${error.message}); alles bouwen.`);
  const out = process.env.GITHUB_OUTPUT;
  for (const name of Object.keys(COMPONENTS)) {
    console.log(`${name}=true`);
    if (out) appendFileSync(out, `${name}=true\n`);
  }
}
