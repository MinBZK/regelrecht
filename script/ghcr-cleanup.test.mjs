// Pint het gedrag van de GHCR-opruimer vast.
//
// Dit script verwijdert onomkeerbaar manifesten uit een productieregistry, dus
// de tests hieronder zijn vooral tests op de *veilige* kant: welke gevallen
// mogen absoluut níet in de deletelijst belanden. Een fout die te weinig
// opruimt kost opslag; een fout die te veel opruimt breekt de draaiende
// productie-image, en dat is niet terug te draaien.
//
// De echte registry komt hier niet aan te pas: `collectReferencedDigests` en
// `planPackage` krijgen hun manifesten via een injecteerbare fetch, zodat de
// randgevallen (mislukte lookup, geneste index, verdwenen digest) die je in
// het wild niet op commando kunt oproepen wél gedekt zijn.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  DEFAULT_GRACE_HOURS,
  HttpError,
  RateLimitError,
  assertNoReferencedDeletions,
  collectReferencedDigests,
  formatSummary,
  graphGaps,
  isIndexMediaType,
  isManifestLike,
  mapLimit,
  nextLink,
  parseArgs,
  planPackage,
  rateLimitOf,
  tagsOf,
} from './ghcr-cleanup.mjs';

const HOUR = 3600 * 1000;
const NOW = Date.parse('2026-08-03T03:00:00Z');
const OLD = '2026-03-25T12:00:00Z';

let nextId = 1;
const version = (digest, tags = [], created_at = OLD) => ({
  id: nextId++,
  name: digest,
  created_at,
  metadata: { container: { tags } },
});

// Een getagde index met twee untagged children, zoals buildx met provenance ze
// produceert: het platform-image en een attestation-manifest.
const buildxIndex = (children) => ({
  mediaType: 'application/vnd.oci.image.index.v1+json',
  manifests: children.map((digest) => ({
    mediaType: 'application/vnd.oci.image.manifest.v1+json',
    digest,
  })),
});

const plan = (overrides) =>
  planPackage({
    packageName: 'regelrecht-editor',
    versions: [],
    openPrs: [],
    referenced: new Set(),
    gaps: [],
    now: NOW,
    graceMs: DEFAULT_GRACE_HOURS * HOUR,
    ...overrides,
  });

test('tagsOf overleeft versies zonder metadata', () => {
  assert.deepEqual(tagsOf({}), []);
  assert.deepEqual(tagsOf({ metadata: {} }), []);
  assert.deepEqual(tagsOf({ metadata: { container: { tags: ['latest'] } } }), ['latest']);
});

test('collectReferencedDigests verzamelt de children van elke getagde index', async () => {
  const manifests = {
    'sha256:idxA': buildxIndex(['sha256:amdA', 'sha256:attA']),
    'sha256:idxB': buildxIndex(['sha256:amdB', 'sha256:attB']),
  };
  const { referenced, unresolved, missing } = await collectReferencedDigests({
    roots: ['sha256:idxA', 'sha256:idxB'],
    fetchManifest: async (digest) => manifests[digest],
  });

  assert.deepEqual([...referenced].sort(), ['sha256:amdA', 'sha256:amdB', 'sha256:attA', 'sha256:attB']);
  assert.deepEqual(unresolved, []);
  assert.deepEqual(missing, []);
});

test('collectReferencedDigests haalt elke digest maar één keer op', async () => {
  const calls = [];
  await collectReferencedDigests({
    roots: ['sha256:idx', 'sha256:idx'],
    fetchManifest: async (digest) => {
      calls.push(digest);
      return buildxIndex(['sha256:child']);
    },
  });
  assert.deepEqual(calls, ['sha256:idx']);
});

test('collectReferencedDigests volgt een geneste index, maar niet de bladeren', async () => {
  // Een index die naar een andere index wijst: de kleinkinderen zijn óók
  // gerefereerd. Zouden we alleen één niveau diep kijken, dan stond het
  // kleinkind straks als wees in de deletelijst.
  const fetched = [];
  const manifests = {
    'sha256:root': {
      mediaType: 'application/vnd.oci.image.index.v1+json',
      manifests: [
        { mediaType: 'application/vnd.oci.image.index.v1+json', digest: 'sha256:nested' },
        { mediaType: 'application/vnd.oci.image.manifest.v1+json', digest: 'sha256:leaf' },
      ],
    },
    'sha256:nested': buildxIndex(['sha256:grandchild']),
  };
  const { referenced } = await collectReferencedDigests({
    roots: ['sha256:root'],
    fetchManifest: async (digest) => {
      fetched.push(digest);
      return manifests[digest];
    },
  });

  assert.ok(referenced.has('sha256:grandchild'));
  assert.deepEqual([...referenced].sort(), ['sha256:grandchild', 'sha256:leaf', 'sha256:nested']);
  // Een blad-descriptor zegt al dat het geen index is; die ophalen zou per
  // package duizenden overbodige requests kosten.
  assert.deepEqual(fetched, ['sha256:root', 'sha256:nested']);
});

test('collectReferencedDigests meldt een mislukte lookup in plaats van hem te negeren', async () => {
  const { referenced, unresolved } = await collectReferencedDigests({
    roots: ['sha256:ok', 'sha256:stuk'],
    fetchManifest: async (digest) => {
      if (digest === 'sha256:stuk') throw new Error('connectie verbroken');
      return buildxIndex(['sha256:child']);
    },
  });

  assert.deepEqual([...referenced], ['sha256:child']);
  assert.equal(unresolved.length, 1);
  assert.equal(unresolved[0].digest, 'sha256:stuk');
  assert.match(unresolved[0].error, /connectie verbroken/);
});

test('collectReferencedDigests telt een 404 apart, maar wel als gat in de graaf', async () => {
  const graph = await collectReferencedDigests({
    roots: ['sha256:weg'],
    fetchManifest: async () => {
      throw new HttpError(404, 'ghcr', 'MANIFEST_UNKNOWN');
    },
  });
  assert.deepEqual(graph.unresolved, []);
  assert.deepEqual(graph.missing, ['sha256:weg']);
  // Een 404 betekent iets anders dan een mislukte lookup en wordt daarom apart
  // gerapporteerd, maar hij blokkeert net zo hard. Zie de test hieronder.
  assert.match(graphGaps(graph).join(' '), /404/);
});

test('een registry die overal 404 geeft blokkeert de wezen-tak', async () => {
  // Het gevaarlijkste scenario van dit script, en de reden dat een 404 niet als
  // "die versie is weg, dus die beschermt niets meer" mag worden afgedaan: GHCR
  // antwoordt op een repo die niet bestaat met exact dezelfde
  // `404 MANIFEST_UNKNOWN` als op een digest die niet bestaat. Eén verkeerd
  // registry-pad — hernoemd package, andere org — laat dus élke lookup 404
  // geven. Zonder dit hek blijft er een lege referentieverzameling over,
  // belanden alle nog levende children als "wees" in de deletelijst, en laat de
  // eindcontrole ze door omdat die tegen precies die lege verzameling toetst.
  const versions = [
    version('sha256:idxProd', ['latest']),
    version('sha256:amdProd'),
    version('sha256:attProd'),
    version('sha256:echteWees'),
  ];
  const graph = await collectReferencedDigests({
    roots: ['sha256:idxProd'],
    fetchManifest: async () => {
      throw new HttpError(404, 'ghcr.io/v2/minbzk/typo/manifests/…', 'MANIFEST_UNKNOWN');
    },
  });
  const result = plan({ versions, referenced: graph.referenced, gaps: graphGaps(graph) });

  assert.equal(result.blocked, true);
  assert.deepEqual(result.deletions, []);
});

test('collectReferencedDigests vertrouwt een 200 die geen manifest is niet', async () => {
  // Een foutobject of loginpagina die toevallig als JSON binnenkomt levert
  // anders stilletjes "nul children" op, en dan zijn de echte children van deze
  // getagde versie ineens wezen.
  const graph = await collectReferencedDigests({
    roots: ['sha256:raar'],
    fetchManifest: async () => ({ errors: [{ code: 'DENIED' }] }),
  });

  assert.equal(graph.referenced.size, 0);
  assert.equal(graph.unresolved.length, 1);
  assert.match(graph.unresolved[0].error, /geen manifest/);
  assert.ok(graphGaps(graph).length > 0);
});

test('graphGaps slaat alarm als geen enkele getagde versie een child oplevert', () => {
  // Zonder dit hek is de eindcontrole vleugellam: die toetst de deletelijst
  // tegen een lege verzameling en laat dus alles door.
  assert.deepEqual(graphGaps({ rootCount: 0, referenced: new Set(), unresolved: [], missing: [] }), []);
  assert.equal(
    graphGaps({ rootCount: 1875, referenced: new Set(), unresolved: [], missing: [] }).length,
    1,
  );
  assert.deepEqual(
    graphGaps({ rootCount: 2, referenced: new Set(['sha256:child']), unresolved: [], missing: [] }),
    [],
  );
});

test('isManifestLike accepteert een lege index maar geen willekeurig object', () => {
  assert.equal(isManifestLike({ manifests: [] }), true);
  assert.equal(isManifestLike({ layers: [], config: {} }), true);
  assert.equal(isManifestLike({ fsLayers: [] }), true);
  assert.equal(isManifestLike({ errors: [] }), false);
  assert.equal(isManifestLike(null), false);
  assert.equal(isManifestLike('sha256:x'), false);
});

test('collectReferencedDigests laat een rate limit doorslaan naar de aanroeper', async () => {
  await assert.rejects(
    collectReferencedDigests({
      roots: ['sha256:a'],
      fetchManifest: async () => {
        throw new RateLimitError('te veel');
      },
    }),
    RateLimitError,
  );
});

test('planPackage verwijdert alleen untagged versies waar niets naar wijst', () => {
  const referenced = new Set(['sha256:amd', 'sha256:att']);
  const result = plan({
    versions: [
      version('sha256:idx', ['latest']),
      version('sha256:amd'),
      version('sha256:att'),
      version('sha256:wees'),
    ],
    referenced,
  });

  assert.deepEqual(result.deletions.map((d) => d.digest), ['sha256:wees']);
  assert.equal(result.counts.tagged, 1);
  assert.equal(result.counts.untagged, 3);
  assert.equal(result.counts.referencedUntagged, 2);
  assert.equal(result.counts.orphaned, 1);
});

test('planPackage raakt geen enkele getagde versie zonder pr-tag', () => {
  const result = plan({
    versions: [version('sha256:a', ['latest']), version('sha256:b', ['sha-abc1234'])],
  });
  assert.deepEqual(result.deletions, []);
});

test('planPackage verwijdert pr-images van gesloten PRs en laat open PRs staan', () => {
  const result = plan({
    versions: [
      version('sha256:open', ['pr-1090', 'sha-aaa1111']),
      version('sha256:dicht', ['pr-999', 'sha-bbb2222']),
    ],
    openPrs: [1090, 1091],
  });

  assert.deepEqual(result.deletions.map((d) => d.digest), ['sha256:dicht']);
  assert.equal(result.deletions[0].reason, 'stale-pr');
});

test('planPackage beschermt latest, ook als er een stale pr-tag op zit', () => {
  const result = plan({
    versions: [version('sha256:prod', ['latest', 'pr-999'])],
    openPrs: [],
  });
  assert.deepEqual(result.deletions, []);
});

test('planPackage laat verse untagged versies staan', () => {
  // De race die dit afvangt: een push die binnenkomt terwijl wij de listing
  // doorlopen. De children staan er dan al wel, de getagde index nog niet.
  const result = plan({
    versions: [
      version('sha256:vers', [], new Date(NOW - 2 * HOUR).toISOString()),
      version('sha256:oud', [], new Date(NOW - 48 * HOUR).toISOString()),
    ],
  });

  assert.deepEqual(result.deletions.map((d) => d.digest), ['sha256:oud']);
  assert.equal(result.counts.skippedTooRecent, 1);
});

test('planPackage behandelt een onleesbare datum als te vers', () => {
  const result = plan({ versions: [version('sha256:raar', [], 'geen datum')] });
  assert.deepEqual(result.deletions, []);
  assert.equal(result.counts.skippedTooRecent, 1);
});

test('planPackage verwijdert geen wezen bij een onvolledige referentiegraaf', () => {
  const result = plan({
    versions: [version('sha256:wees'), version('sha256:dicht', ['pr-999'])],
    gaps: ['1 manifest-lookup(s) mislukt'],
  });

  assert.equal(result.blocked, true);
  // De wees blijft staan: hij kan een child zijn van precies het manifest dat
  // we niet konden lezen. De pr-tak raakt alleen getagde versies en gaat door.
  assert.deepEqual(result.deletions.map((d) => d.reason), ['stale-pr']);
  assert.equal(result.counts.orphaned, 1);
});

test('assertNoReferencedDeletions stopt een deletelijst met een gerefereerde digest', () => {
  const referenced = new Set(['sha256:amd']);
  assert.ok(assertNoReferencedDeletions([{ digest: 'sha256:wees' }], referenced));
  assert.throws(
    () => assertNoReferencedDeletions([{ digest: 'sha256:amd' }], referenced),
    /veiligheidscontrole gefaald/,
  );
});

test('een realistisch package levert precies de losgeraakte children op', async () => {
  // Twee builds van hetzelfde image: `latest` is verschoven van de eerste
  // index naar de tweede. De children van de eerste index zijn daarmee wees
  // geworden; die van de tweede horen bij wat er in productie draait.
  const versions = [
    version('sha256:idx2', ['latest', 'sha-2222222']),
    version('sha256:idx1', ['sha-1111111']),
    version('sha256:amd2'),
    version('sha256:att2'),
    version('sha256:amd1'),
    version('sha256:att1'),
    version('sha256:wees'),
  ];
  const manifests = {
    'sha256:idx2': buildxIndex(['sha256:amd2', 'sha256:att2']),
    'sha256:idx1': buildxIndex(['sha256:amd1', 'sha256:att1']),
  };

  const roots = versions.filter((v) => tagsOf(v).length > 0).map((v) => v.name);
  const graph = await collectReferencedDigests({ roots, fetchManifest: async (digest) => manifests[digest] });
  const { referenced } = graph;
  const result = plan({ versions, referenced, gaps: graphGaps(graph) });

  // `sha-1111111` blijft getagd, dus zijn children blijven staan: retentie
  // voor oude sha-tags valt buiten deze opruimer.
  assert.deepEqual(result.deletions.map((d) => d.digest), ['sha256:wees']);
  assert.ok(assertNoReferencedDeletions(result.deletions, referenced));
});

const summaryResult = (overrides = {}) => ({
  packageName: 'regelrecht-docs',
  counts: {
    total: 10,
    tagged: 4,
    untagged: 6,
    referencedDigests: 8,
    referencedUntagged: 5,
    orphaned: 1,
    skippedTooRecent: 0,
    stalePr: 2,
  },
  blocked: false,
  gaps: [],
  referencedInDeletions: 0,
  deletions: [{ digest: 'sha256:wees' }, { digest: 'sha256:oud' }, { digest: 'sha256:dicht' }],
  deleted: 0,
  failures: [],
  ...overrides,
});

test('formatSummary noemt geblokkeerde packages en mislukte verwijderingen', () => {
  const summary = formatSummary(
    [
      summaryResult({
        blocked: true,
        gaps: ['1 manifest-lookup(s) mislukt (bv. sha256:x — HTTP 500)'],
        deleted: 1,
        failures: [{ id: 42, digest: 'sha256:y', error: 'HTTP 403' }],
      }),
      { packageName: 'regelrecht-admin', error: 'HTTP 502' },
    ],
    { dryRun: false },
  );

  assert.match(summary, /regelrecht-docs/);
  assert.match(summary, /Onvolledige referentiegraaf/);
  assert.match(summary, /sha256:x/);
  assert.match(summary, /Mislukte verwijderingen/);
  assert.match(summary, /sha256:y/);
  assert.match(summary, /overgeslagen: HTTP 502/);
});

test('formatSummary rapporteert in dry-run wat er te verwijderen valt', () => {
  // Criterium van het ticket: een dry-run moet per package zeggen hoeveel er
  // gerefereerd, verweesd en te verwijderen is. Zonder dit stond er in de
  // kolom "verwijderd" een 0 die niets zei over de omvang van het plan.
  const summary = formatSummary([summaryResult()], { dryRun: true });

  assert.match(summary, /dry-run/);
  assert.match(summary, /te verwijderen/);
  // 3 geplande verwijderingen, en geen verwarrend aantal daadwerkelijk gewiste
  // versies: dat is in een dry-run niet van toepassing.
  assert.match(summary, /\| 3 \| — \| 0 \|/);
  assert.match(summary, /8 gerefereerde digest\(s\).*\*\*0\*\*.*3 geplande verwijdering/s);
});

test('formatSummary meldt een gerefereerde digest in de deletelijst in plaats van 0 te beloven', () => {
  // De samenvatting is het bewijs waarmee criterium 4 wordt afgevinkt. Zou dat
  // getal vastgeschroefd op 0 staan, dan zou juist de run die je had willen
  // zien — de veiligheidscontrole slaat aan — netjes "0" blijven melden.
  const summary = formatSummary([summaryResult({ referencedInDeletions: 2 })], { dryRun: true });
  assert.match(summary, /daarvan staan er \*\*2\*\* in de 3 geplande verwijdering/);
});

test('formatSummary telt de gerefereerde digests over alle packages op', () => {
  const summary = formatSummary(
    [summaryResult(), summaryResult({ packageName: 'regelrecht-admin' }), { packageName: 'x', error: 'stuk' }],
    { dryRun: true },
  );
  assert.match(summary, /16 gerefereerde digest\(s\)/);
  assert.match(summary, /6 geplande verwijdering/);
});

test('parseArgs leest de vlaggen en weigert onzin', () => {
  assert.equal(parseArgs(['--dry-run']).dryRun, true);
  assert.deepEqual(parseArgs(['--packages', 'a, b']).packages, ['a', 'b']);
  assert.equal(parseArgs(['--grace-hours', '0']).graceHours, 0);
  assert.throws(() => parseArgs(['--onzin']), /onbekend argument/);
  assert.throws(() => parseArgs(['--grace-hours', '-1']), /niet-negatief/);
  assert.throws(() => parseArgs(['--concurrency', '0']), /positief/);
});

test('rateLimitOf herkent ook de secondary rate limit', () => {
  const res = (status, headers = {}) => ({ status, headers: new Map(Object.entries(headers)) });

  // De variant die dit script het hardst raakt: duizenden lookups achter elkaar
  // leveren een 403 + Retry-After op, terwijl het primaire quotum niet op is en
  // `x-ratelimit-remaining` dus gewoon hoog staat.
  assert.deepEqual(
    rateLimitOf(res(403, { 'retry-after': '30', 'x-ratelimit-remaining': '4231' })),
    { retryAfterSeconds: 30 },
  );
  // Primair quotum op.
  assert.deepEqual(rateLimitOf(res(403, { 'x-ratelimit-remaining': '0' })), { retryAfterSeconds: 0 });
  // 429 is altijd een limiet, met of zonder headers.
  assert.deepEqual(rateLimitOf(res(429, {})), { retryAfterSeconds: 0 });
  // Een gewone 403 (token mist een scope) blijft een harde fout, geen limiet.
  assert.equal(rateLimitOf(res(403, { 'x-ratelimit-remaining': '4231' })), null);
  assert.equal(rateLimitOf(res(404, {})), null);
  assert.equal(rateLimitOf(res(200, {})), null);
});

test('nextLink pakt alleen de next-relatie', () => {
  const header =
    '<https://api.github.com/orgs/x/packages?page=2>; rel="next", <https://api.github.com/orgs/x/packages?page=9>; rel="last"';
  assert.equal(nextLink(header), '/orgs/x/packages?page=2');
  assert.equal(nextLink('<https://api.github.com/x?page=1>; rel="prev"'), null);
  assert.equal(nextLink(null), null);
});

test('isIndexMediaType kent alleen de indextypes', () => {
  assert.equal(isIndexMediaType('application/vnd.oci.image.index.v1+json'), true);
  assert.equal(isIndexMediaType('application/vnd.docker.distribution.manifest.list.v2+json'), true);
  assert.equal(isIndexMediaType('application/vnd.oci.image.manifest.v1+json'), false);
  assert.equal(isIndexMediaType(undefined), false);
});

test('mapLimit houdt de volgorde aan en respecteert het plafond', async () => {
  let running = 0;
  let peak = 0;
  const out = await mapLimit([1, 2, 3, 4, 5, 6, 7], 3, async (n) => {
    running++;
    peak = Math.max(peak, running);
    await new Promise((resolve) => setTimeout(resolve, 1));
    running--;
    return n * 2;
  });

  assert.deepEqual(out, [2, 4, 6, 8, 10, 12, 14]);
  assert.ok(peak <= 3, `piek was ${peak}`);
});
