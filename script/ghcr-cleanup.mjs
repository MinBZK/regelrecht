#!/usr/bin/env node
// Ruimt verweesde container-versies op in GHCR, zonder de manifesten te raken
// waar een getagde image nog naar wijst.
//
// Het probleem dat dit script oplost: buildx zet provenance standaard aan, dus
// elke push levert een OCI *index* met de tag plus twee untagged children (het
// platform-image en een attestation-manifest). Van de untagged versies in een
// package is daardoor het merendeel geen afval maar een levend onderdeel van
// een getagde image. Een opruimer die "alles zonder tag" weggooit sloopt dus
// élke image in de registry, inclusief wat er in productie draait. De vorige
// opruimstap ging de andere kant op en sloeg untagged versies volledig over,
// waardoor de echte wezen — children waar geen tag meer naar wijst omdat
// `latest` of `pr-N` naar een nieuwe index is verschoven — voor onbepaalde
// tijd bleven staan.
//
// De aanpak is daarom: eerst de referentiegraaf opbouwen (per getagde versie
// het manifest ophalen en alle `.manifests[].digest` verzamelen), en pas
// daarna de untagged versies verwijderen die in die verzameling ontbreken.
//
// Dit is destructieve code tegen een productieregistry, dus elke twijfel valt
// de kant van "niets verwijderen" op:
//
//   * Kan een manifest van een getagde versie niet worden opgehaald, dan is de
//     graaf onvolledig en wordt er voor dat package géén enkele wees
//     verwijderd. Liever een ronde niets opruimen dan één keer te veel weg.
//   * Untagged versies jonger dan de respijttermijn blijven staan. Een push
//     die tijdens deze run binnenkomt kan children hebben die in onze listing
//     nog geen ouder hebben.
//   * Versies met een beschermde tag (`latest`) worden nooit verwijderd, ook
//     niet als er verder een stale `pr-*`-tag op zit.
//   * Vlak voor het verwijderen wordt de deletelijst nog een keer tegen de
//     referentieverzameling gehouden. Zit er een gerefereerde digest in, dan
//     stopt de run zonder iets te verwijderen.
//
// Gebruik:
//   GH_TOKEN=… node script/ghcr-cleanup.mjs --dry-run
//   GH_TOKEN=… node script/ghcr-cleanup.mjs --packages regelrecht-docs
//
// Het token heeft `read:packages` nodig om te plannen en `delete:packages` om
// daadwerkelijk op te ruimen.

import { appendFileSync, realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// De containerpackages die deze repo naar GHCR schrijft. Staat hier en niet in
// de workflow, zodat de lijst en de logica die hem gebruikt bij elkaar blijven.
export const PACKAGES = [
  'regelrecht-editor',
  'regelrecht-admin',
  'regelrecht-harvester-worker',
  'regelrecht-enrich-worker',
  'regelrecht-lawmaking',
  'regelrecht-docs',
];

// Tags die een versie onaanraakbaar maken, wat er verder ook op zit. `latest`
// is waar de productie-deploy naar wijst.
export const PROTECTED_TAGS = new Set(['latest']);

// Een untagged versie die jonger is dan dit blijft staan. Vangt de race tussen
// onze versions-listing en een push die tijdens de run binnenkomt: de children
// van die push zijn dan al wel gelist, maar de index met de tag nog niet.
export const DEFAULT_GRACE_HOURS = 24;

const INDEX_MEDIA_TYPES = new Set([
  'application/vnd.oci.image.index.v1+json',
  'application/vnd.docker.distribution.manifest.list.v2+json',
]);

const MANIFEST_ACCEPT = [
  'application/vnd.oci.image.index.v1+json',
  'application/vnd.oci.image.manifest.v1+json',
  'application/vnd.docker.distribution.manifest.list.v2+json',
  'application/vnd.docker.distribution.manifest.v2+json',
].join(', ');

export class RateLimitError extends Error {
  constructor(message) {
    super(message);
    this.name = 'RateLimitError';
  }
}

export class HttpError extends Error {
  constructor(status, url, body) {
    super(`HTTP ${status} voor ${url}${body ? `: ${body}` : ''}`);
    this.name = 'HttpError';
    this.status = status;
  }
}

// --- pure logica (getest in ghcr-cleanup.test.mjs) ---------------------------

export const tagsOf = (version) => version?.metadata?.container?.tags ?? [];

export const isIndexMediaType = (mediaType) => INDEX_MEDIA_TYPES.has(mediaType);

/**
 * Bouwt de verzameling digests waar een getagde versie naar wijst.
 *
 * `roots` zijn de digests van de getagde versies; `fetchManifest(digest)` levert
 * het geparste manifest of gooit. Een index kan in theorie naar een andere
 * index wijzen, dus de wandeling is transitief — maar alleen langs children
 * waarvan de descriptor al zegt dat het een index is. Dat kost geen extra
 * requests voor de image- en attestation-manifesten die buildx in de praktijk
 * produceert, en dekt de geneste variant toch af.
 *
 * Een manifest dat niet op te halen is komt in `unresolved`. De aanroeper mag
 * dan niets verwijderen: de graaf is onvolledig en elke "wees" kan een child
 * zijn van precies het manifest dat we niet konden lezen.
 */
export async function collectReferencedDigests({ roots, fetchManifest, concurrency = 8 }) {
  const referenced = new Set();
  const unresolved = [];
  const missing = [];
  const visited = new Set();
  let frontier = [...new Set(roots)];

  while (frontier.length > 0) {
    const next = [];
    await mapLimit(frontier, concurrency, async (digest) => {
      if (visited.has(digest)) return;
      visited.add(digest);

      let manifest;
      try {
        manifest = await fetchManifest(digest);
      } catch (err) {
        if (err instanceof RateLimitError) throw err;
        // Een 404 betekent dat de versie tussen listing en lookup is
        // verdwenen. Die kan dus ook niets meer beschermen; dat is geen
        // onzekerheid maar een feit.
        if (err instanceof HttpError && err.status === 404) {
          missing.push(digest);
          return;
        }
        unresolved.push({ digest, error: err?.message ?? String(err) });
        return;
      }

      for (const child of manifest?.manifests ?? []) {
        if (!child?.digest) continue;
        referenced.add(child.digest);
        if (isIndexMediaType(child.mediaType) && !visited.has(child.digest)) {
          next.push(child.digest);
        }
      }
    });
    frontier = next;
  }

  return { referenced, unresolved, missing };
}

/**
 * Bepaalt wat er voor één package weg mag.
 *
 * Twee soorten kandidaten: getagde `pr-*`-versies van gesloten PR's (het
 * bestaande gedrag) en untagged versies waar geen getagde index naar wijst (de
 * wezen). Is de referentiegraaf onvolledig (`unresolvedCount > 0`), dan
 * vervallen de wezen — de `pr-*`-tak raakt alleen getagde versies en blijft
 * dus wel gewoon werken.
 */
export function planPackage({
  packageName,
  versions,
  openPrs,
  referenced,
  unresolvedCount = 0,
  now = Date.now(),
  graceMs = DEFAULT_GRACE_HOURS * 3600 * 1000,
}) {
  const openPrSet = new Set([...openPrs].map(String));
  const tagged = [];
  const untagged = [];
  for (const version of versions) {
    (tagsOf(version).length > 0 ? tagged : untagged).push(version);
  }

  const protectedVersions = tagged.filter((v) => tagsOf(v).some((t) => PROTECTED_TAGS.has(t)));
  const protectedIds = new Set(protectedVersions.map((v) => v.id));

  // Een versie telt als stale PR-image wanneer er minstens één `pr-*`-tag op
  // zit en géén daarvan bij een open PR hoort. Verwijderen haalt ook de
  // meeliftende `sha-*`-tag weg; dat was al zo en is ook de bedoeling.
  const stalePr = tagged.filter((version) => {
    if (protectedIds.has(version.id)) return false;
    const prNumbers = tagsOf(version)
      .filter((t) => t.startsWith('pr-'))
      .map((t) => t.slice('pr-'.length));
    return prNumbers.length > 0 && prNumbers.every((n) => !openPrSet.has(n));
  });

  const referencedUntagged = untagged.filter((v) => referenced.has(v.name));
  const candidates = untagged.filter((v) => !referenced.has(v.name));

  const orphans = [];
  const tooRecent = [];
  for (const version of candidates) {
    const age = now - Date.parse(version.created_at);
    // Een onleesbare datum telt als "te vers": onbekend is geen vrijbrief.
    (Number.isFinite(age) && age >= graceMs ? orphans : tooRecent).push(version);
  }

  const blocked = unresolvedCount > 0;
  const deletions = [
    ...stalePr.map((version) => ({
      id: version.id,
      digest: version.name,
      tags: tagsOf(version),
      reason: 'stale-pr',
    })),
    ...(blocked
      ? []
      : orphans.map((version) => ({
          id: version.id,
          digest: version.name,
          tags: [],
          reason: 'orphan',
        }))),
  ];

  return {
    packageName,
    blocked,
    counts: {
      total: versions.length,
      tagged: tagged.length,
      untagged: untagged.length,
      referencedDigests: referenced.size,
      referencedUntagged: referencedUntagged.length,
      orphaned: orphans.length,
      skippedTooRecent: tooRecent.length,
      stalePr: stalePr.length,
    },
    deletions,
  };
}

/**
 * Laatste hek voor de DELETE's: geen enkele digest in de deletelijst mag in de
 * referentieverzameling zitten. Gooit met de overtreders, zodat een run die
 * hier struikelt niets verwijdert in plaats van het verkeerde.
 */
export function assertNoReferencedDeletions(deletions, referenced) {
  const violations = deletions.filter((d) => referenced.has(d.digest));
  if (violations.length > 0) {
    throw new Error(
      `veiligheidscontrole gefaald: ${violations.length} gerefereerde digest(s) in de deletelijst ` +
        `(${violations.slice(0, 5).map((v) => v.digest).join(', ')})`,
    );
  }
  return true;
}

export function formatSummary(results, { dryRun }) {
  const lines = [];
  lines.push(`## GHCR-opruiming${dryRun ? ' (dry-run)' : ''}`, '');
  lines.push(
    '| package | versies | getagd | untagged | gerefereerd | wees | te vers | stale pr-* | te verwijderen | verwijderd | fouten |',
  );
  lines.push('|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|');
  for (const result of results) {
    if (result.error) {
      lines.push(`| ${result.packageName} | overgeslagen: ${result.error} | | | | | | | | | |`);
      continue;
    }
    const c = result.counts;
    lines.push(
      `| ${result.packageName} | ${c.total} | ${c.tagged} | ${c.untagged} | ${c.referencedUntagged} | ` +
        `${c.orphaned} | ${c.skippedTooRecent} | ${c.stalePr} | ${result.deletions?.length ?? 0} | ` +
        `${dryRun ? '—' : (result.deleted ?? 0)} | ${result.failures?.length ?? 0} |`,
    );
  }
  lines.push('');

  // Het bewijs dat de opruimer de referentiegraaf respecteert hoort in de
  // samenvatting zelf te staan, niet alleen in de exitcode: per package het
  // aantal gerefereerde digests naast de belofte dat er geen enkele in de
  // deletelijst zit.
  const planned = results.filter((r) => !r.error);
  if (planned.length > 0) {
    const totalReferenced = planned.reduce((sum, r) => sum + r.counts.referencedDigests, 0);
    const totalDeletions = planned.reduce((sum, r) => sum + (r.deletions?.length ?? 0), 0);
    lines.push(
      `${totalReferenced} gerefereerde digest(s) in kaart gebracht; ` +
        `daarvan staat er **0** in de ${totalDeletions} geplande verwijdering(en).`,
      '',
    );
  }

  const blocked = results.filter((r) => r.blocked);
  if (blocked.length > 0) {
    lines.push('### Onvolledige referentiegraaf', '');
    lines.push('Voor deze packages kon niet elk getagd manifest worden opgehaald. Er zijn geen wezen verwijderd:', '');
    for (const result of blocked) {
      lines.push(`- **${result.packageName}**: ${result.unresolved.length} mislukte manifest-lookup(s), ` +
        `bv. \`${result.unresolved[0]?.digest}\` — ${result.unresolved[0]?.error}`);
    }
    lines.push('');
  }

  const withFailures = results.filter((r) => (r.failures?.length ?? 0) > 0);
  if (withFailures.length > 0) {
    lines.push('### Mislukte verwijderingen', '');
    for (const result of withFailures) {
      for (const failure of result.failures) {
        lines.push(`- \`${result.packageName}\` versie ${failure.id} (${failure.digest}): ${failure.error}`);
      }
    }
    lines.push('');
  }

  return lines.join('\n');
}

// --- HTTP ---------------------------------------------------------------

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export async function mapLimit(items, limit, worker) {
  const results = new Array(items.length);
  let index = 0;
  const runners = Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, async () => {
    while (index < items.length) {
      const current = index++;
      results[current] = await worker(items[current], current);
    }
  });
  await Promise.all(runners);
  return results;
}

// Een enkele call mag nooit oneindig blijven hangen: deze run doet er duizenden
// en draait onbewaakt in een nachtelijke pipeline.
const REQUEST_TIMEOUT_MS = 30_000;

// Haalt een URL op met backoff op 429 en 5xx. Een 429 die na alle pogingen
// blijft staan is geen incident maar een muur: dan stopt de run liever dan dat
// hij met een halve graaf verder rekent.
async function request(url, { method = 'GET', headers = {}, attempts = 5 } = {}) {
  let lastError;
  for (let attempt = 0; attempt < attempts; attempt++) {
    if (attempt > 0) await sleep(Math.min(30_000, 1000 * 2 ** (attempt - 1)));

    let response;
    try {
      response = await fetch(url, { method, headers, signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS) });
    } catch (err) {
      lastError = err;
      continue;
    }

    if (response.status === 429 || response.status === 403) {
      const remaining = response.headers.get('x-ratelimit-remaining');
      const isRateLimited = response.status === 429 || remaining === '0';
      if (isRateLimited) {
        if (attempt === attempts - 1) {
          throw new RateLimitError(`rate limit bereikt op ${url} (status ${response.status})`);
        }
        const retryAfter = Number(response.headers.get('retry-after'));
        await sleep(Number.isFinite(retryAfter) && retryAfter > 0 ? retryAfter * 1000 : 0);
        continue;
      }
    }

    if (response.status >= 500) {
      lastError = new HttpError(response.status, url, (await response.text()).slice(0, 200));
      continue;
    }

    return response;
  }
  throw lastError ?? new Error(`geen antwoord van ${url}`);
}

function createClient({ token, org }) {
  const apiHeaders = {
    accept: 'application/vnd.github+json',
    authorization: `Bearer ${token}`,
    'x-github-api-version': '2022-11-28',
    'user-agent': 'regelrecht-ghcr-cleanup',
  };
  // De registry-API verwacht hetzelfde token, maar base64 als basic-credential
  // in een bearer verpakt. Zonder die omweg krijg je een 401.
  const registryBearer = Buffer.from(`x:${token}`).toString('base64');
  const registryOrg = org.toLowerCase();

  const api = async (path, init) => {
    const response = await request(`https://api.github.com${path}`, { ...init, headers: apiHeaders });
    if (!response.ok) {
      throw new HttpError(response.status, path, (await response.text()).slice(0, 200));
    }
    return response;
  };

  return {
    async listPackageVersions(packageName) {
      const versions = [];
      let path = `/orgs/${org}/packages/container/${encodeURIComponent(packageName)}/versions?per_page=100`;
      while (path) {
        const response = await api(path);
        versions.push(...(await response.json()));
        path = nextLink(response.headers.get('link'));
      }
      return versions;
    },

    async listOpenPullRequests(repo) {
      const numbers = [];
      let path = `/repos/${repo}/pulls?state=open&per_page=100`;
      while (path) {
        const response = await api(path);
        for (const pr of await response.json()) numbers.push(pr.number);
        path = nextLink(response.headers.get('link'));
      }
      return numbers;
    },

    async fetchManifest(packageName, reference) {
      const url = `https://ghcr.io/v2/${registryOrg}/${packageName}/manifests/${reference}`;
      const response = await request(url, {
        headers: { authorization: `Bearer ${registryBearer}`, accept: MANIFEST_ACCEPT },
      });
      if (!response.ok) {
        throw new HttpError(response.status, url, (await response.text()).slice(0, 200));
      }
      return response.json();
    },

    async deleteVersion(packageName, versionId) {
      await api(`/orgs/${org}/packages/container/${encodeURIComponent(packageName)}/versions/${versionId}`, {
        method: 'DELETE',
      });
    },
  };
}

export function nextLink(linkHeader) {
  if (!linkHeader) return null;
  for (const part of linkHeader.split(',')) {
    const match = part.match(/<([^>]+)>\s*;\s*rel="next"/);
    if (match) return match[1].replace('https://api.github.com', '');
  }
  return null;
}

// --- CLI ----------------------------------------------------------------

export function parseArgs(argv) {
  const options = {
    dryRun: false,
    packages: PACKAGES,
    graceHours: DEFAULT_GRACE_HOURS,
    concurrency: 8,
  };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--dry-run') options.dryRun = true;
    else if (arg === '--packages') options.packages = argv[++i].split(',').map((s) => s.trim()).filter(Boolean);
    else if (arg === '--grace-hours') options.graceHours = Number(argv[++i]);
    else if (arg === '--concurrency') options.concurrency = Number(argv[++i]);
    else throw new Error(`onbekend argument: ${arg}`);
  }
  if (!Number.isFinite(options.graceHours) || options.graceHours < 0) {
    throw new Error('--grace-hours verwacht een niet-negatief getal');
  }
  if (!Number.isInteger(options.concurrency) || options.concurrency < 1) {
    throw new Error('--concurrency verwacht een positief geheel getal');
  }
  return options;
}

const log = (message) => process.stdout.write(`${message}\n`);
const annotate = (level, message) => log(`::${level}::${message.replace(/\n/g, '%0A')}`);

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const dryRun = options.dryRun || process.env.DRY_RUN === 'true';
  const token = process.env.GH_TOKEN;
  const org = process.env.ORG || process.env.GITHUB_REPOSITORY_OWNER;
  const repo = process.env.GITHUB_REPOSITORY || (org ? `${org}/regelrecht` : null);

  if (!token) throw new Error('GH_TOKEN ontbreekt');
  if (!org) throw new Error('ORG of GITHUB_REPOSITORY_OWNER ontbreekt');

  const client = createClient({ token, org });
  const graceMs = options.graceHours * 3600 * 1000;

  log(`Opruiming van ${options.packages.length} package(s) in ${org}${dryRun ? ' — dry-run' : ''}`);
  const openPrs = await client.listOpenPullRequests(repo);
  log(`Open PR's: ${openPrs.length}`);

  const results = [];
  let sawDeleteFailure = false;
  let sawPackageError = false;

  // Wat er ook misgaat, de samenvatting van wat er tot dat moment is gebeurd
  // moet eruit komen: bij destructief werk is "welke DELETE's zijn al gedaan"
  // precies wat je wilt weten als een run halverwege stopt.
  try {
    packages: for (const packageName of options.packages) {
      log(`\n--- ${packageName}`);
      let versions;
      let graph;
      try {
        versions = await client.listPackageVersions(packageName);
        const roots = versions.filter((v) => tagsOf(v).length > 0).map((v) => v.name);
        log(`  ${versions.length} versies, ${roots.length} getagd — manifesten ophalen...`);
        graph = await collectReferencedDigests({
          roots,
          concurrency: options.concurrency,
          fetchManifest: (digest) => client.fetchManifest(packageName, digest),
        });
      } catch (err) {
        results.push({ packageName, error: err.message });
        sawPackageError = true;
        if (err instanceof RateLimitError) {
          annotate('error', `Rate limit tijdens ${packageName}; run gestopt zonder verder op te ruimen: ${err.message}`);
          break packages;
        }
        annotate('error', `${packageName} overgeslagen: ${err.message}`);
        continue;
      }

      const plan = planPackage({
        packageName,
        versions,
        openPrs,
        referenced: graph.referenced,
        unresolvedCount: graph.unresolved.length,
        graceMs,
      });
      const result = { ...plan, unresolved: graph.unresolved, missing: graph.missing, deleted: 0, failures: [] };
      results.push(result);

      const c = plan.counts;
      log(
        `  totaal ${c.total} | getagd ${c.tagged} | untagged ${c.untagged} | ` +
          `gerefereerd ${c.referencedUntagged} | wees ${c.orphaned} | te vers ${c.skippedTooRecent} | ` +
          `stale pr-* ${c.stalePr} | te verwijderen ${plan.deletions.length}`,
      );
      if (graph.missing.length > 0) log(`  ${graph.missing.length} getagde versie(s) waren al verdwenen (404)`);
      if (plan.blocked) {
        annotate(
          'warning',
          `${packageName}: ${graph.unresolved.length} manifest-lookup(s) mislukt, referentiegraaf onvolledig — ` +
            'geen wezen verwijderd deze ronde',
        );
        for (const item of graph.unresolved.slice(0, 5)) log(`    onopgelost: ${item.digest} — ${item.error}`);
      }

      assertNoReferencedDeletions(plan.deletions, graph.referenced);

      for (const deletion of plan.deletions) {
        const label = `${packageName} ${deletion.digest}${deletion.tags.length ? ` [${deletion.tags.join(', ')}]` : ''} (${deletion.reason})`;
        if (dryRun) {
          log(`  [dry-run] zou verwijderen: ${label}`);
          continue;
        }
        try {
          await client.deleteVersion(packageName, deletion.id);
          result.deleted++;
          log(`  verwijderd: ${label}`);
        } catch (err) {
          // Een losse mislukte DELETE mag de run niet laten falen — die wordt
          // gerapporteerd en volgende ronde opnieuw geprobeerd. Een rate limit
          // is iets anders: dan is elke volgende call ook kansloos en stoppen
          // we liever dan half opgeruimd door te denderen.
          sawDeleteFailure = true;
          result.failures.push({ id: deletion.id, digest: deletion.digest, error: err.message });
          annotate('warning', `DELETE mislukt voor ${label}: ${err.message}`);
          if (err instanceof RateLimitError) {
            annotate('error', `Rate limit tijdens het verwijderen in ${packageName}; run gestopt: ${err.message}`);
            sawPackageError = true;
            break packages;
          }
        }
      }
    }
  } finally {
    // Wat er ook misgaat, de samenvatting van wat er tot dat moment gebeurd is
    // moet eruit komen: bij destructief werk is "welke DELETE's zijn al
    // gedaan" precies wat je wilt weten als een run halverwege stopt.
    log('');
    log(formatSummary(results, { dryRun }));
    writeSummary(results, { dryRun });
  }

  if (sawPackageError) process.exitCode = 1;
  if (sawDeleteFailure) log('Er waren mislukte verwijderingen; zie de samenvatting hierboven.');
}

function writeSummary(results, options) {
  const target = process.env.GITHUB_STEP_SUMMARY;
  if (!target) return;
  appendFileSync(target, `${formatSummary(results, options)}\n`);
}

const invokedDirectly =
  process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
if (invokedDirectly) {
  main().catch((err) => {
    annotate('error', `GHCR-opruiming gestopt: ${err.message}`);
    process.exitCode = 1;
  });
}
