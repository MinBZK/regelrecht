/**
 * Beleidsassistent-backend.
 *
 * POST /api/assistent  { modus: "instructie" | "doel", prompt: string, documenten?: [{ key, yaml }] }
 *   documenten = de werkversie uit de browser (beginstand van de overlays)
 * POST /api/variant    { sleutel, titel, bestanden: [{ path, yaml }] } → branch variant/<sleutel>
 *   → SSE-stream met events:
 *     {type: "tekst", tekst}            assistent-tekst (per beurt)
 *     {type: "tool", naam, input}       toolaanroep van de assistent
 *     {type: "wijziging", document_key, toelichting}
 *     {type: "simulatie", doel, n?, metrics?}
 *     {type: "klaar", overlays: {key: yaml}, handelingen: yaml|null}
 *                                       eindstand van de regelgeving, en van
 *                                       het uitvoeringslastmodel als de
 *                                       assistent dat wijzigde
 *     {type: "fout", melding}
 *
 * Motor: de lokaal geïnstalleerde Claude Code CLI (`claude -p`), headless. Er is
 * dus geen ANTHROPIC_API_KEY nodig; het Claude-abonnement van de gebruiker
 * betaalt. Per request spawnen we een claude-proces met de vier regelrecht-tools
 * uit een stdio-MCP-server (mcp-regelrecht.js) en parsen de stream-json-uitvoer
 * regel voor regel naar de SSE-events hierboven.
 */
import http from 'http';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { spawn, execFile } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PORT = process.env.PORT ? Number(process.env.PORT) : 3600;
// Sonnet en niet opus als standaard: dit draait hosted op één abonnement, en de
// assistent is een demonstratie, geen productiewerk. Te overschrijven.
const MODEL = process.env.ASSISTENT_MODEL ?? 'sonnet';
const CLAUDE_BIN = process.env.CLAUDE_BIN ?? 'claude';

// Welke casus deze assistent bedient. De MCP-tools verschillen echt per casus
// (andere wetten, andere simulatie), de rest van dit bestand niet — vandaar
// één server met een casus-parameter in plaats van twee bijna-kopieën.
const CASUS = process.env.POC_CASUS ?? 'terugbetaalregimes';
const CASUS_MAP = path.resolve(__dirname, 'casus');
const MCP_SERVER = path.resolve(CASUS_MAP, `${CASUS}.mcp.js`);
const PROMPT_BESTAND = path.resolve(CASUS_MAP, `${CASUS}.prompt.txt`);

if (!fs.existsSync(MCP_SERVER) || !fs.existsSync(PROMPT_BESTAND)) {
  // Hard, en bij het opstarten: een assistent zonder zijn eigen tools of
  // systeemprompt praat wel, maar over de verkeerde wet.
  console.error(
    `onbekende casus ${JSON.stringify(CASUS)}: ${MCP_SERVER} of ${PROMPT_BESTAND} ontbreekt`,
  );
  process.exit(1);
}

// "Bewaar als variant" maakt een git-branch in de casus-checkout. Hosted staat
// die er niet (POC_VARIANT_OPSLAG=0 in start.sh), en lokaal staat de vlag ook
// op 0: handleVariant draagt nog de paden van de losse PoC-repo van voor de
// verhuizing naar de monorepo. CASUS_DIR wijst daar naar `packages/`, dus het
// zou schrijven in `packages/corpus/` (een Rust-crate) en daarna een
// copy-assets.js zoeken op `packages/app/scripts/` die niet bestaat.
//
// De route is dus nergens werkend, en een 410 die zegt "kan alleen lokaal" is
// daarmee zelf achterhaald. Varianten bewaren loopt via de browser-opslag.
const VARIANT_OPSLAG = process.env.POC_VARIANT_OPSLAG !== '0';

const SYSTEM = fs.readFileSync(PROMPT_BESTAND, 'utf8');

// ---- CLI-healthcheck (eenmalig gecached) --------------------------------

let claudeCliOk = null;
function checkClaudeCli() {
  return new Promise((resolve) => {
    execFile(CLAUDE_BIN, ['--version'], { timeout: 5000 }, (err) => {
      claudeCliOk = !err;
      resolve(claudeCliOk);
    });
  });
}

// ---- Sessie-map (overlays + events van de MCP-server) -------------------

function maakSessieMap() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ocw-assistent-'));
  fs.mkdirSync(path.join(dir, 'overlays'), { recursive: true });
  fs.mkdirSync(path.join(dir, 'events'), { recursive: true });
  return dir;
}

function leesOverlays(sessieDir) {
  const overlaysDir = path.join(sessieDir, 'overlays');
  const overlays = {};
  if (!fs.existsSync(overlaysDir)) return overlays;
  for (const file of fs.readdirSync(overlaysDir)) {
    if (!file.endsWith('.yaml')) continue;
    const key = decodeURIComponent(file.slice(0, -'.yaml'.length));
    overlays[key] = fs.readFileSync(path.join(overlaysDir, file), 'utf-8');
  }
  return overlays;
}

/**
 * De bewerkte handelingen.yaml van deze sessie, of null als de assistent het
 * uitvoeringslastmodel niet heeft aangeraakt. Ligt naast overlays/ omdat het
 * geen wet is en niet door de engine gaat.
 */
function leesHandelingen(sessieDir) {
  const file = path.join(sessieDir, 'handelingen.yaml');
  return fs.existsSync(file) ? fs.readFileSync(file, 'utf-8') : null;
}

/**
 * Lees nieuwe event-bestanden die de MCP-server heeft neergelegd (oplopend
 * genummerd) en geef ze door. `gezien` houdt bij welke al verstuurd zijn.
 */
function leesNieuweEvents(sessieDir, gezien) {
  const eventsDir = path.join(sessieDir, 'events');
  if (!fs.existsSync(eventsDir)) return [];
  const nieuw = [];
  for (const file of fs.readdirSync(eventsDir).sort()) {
    if (gezien.has(file)) continue;
    gezien.add(file);
    try {
      nieuw.push(JSON.parse(fs.readFileSync(path.join(eventsDir, file), 'utf-8')));
    } catch {
      // half weggeschreven bestand: volgende poll pakt het op
      gezien.delete(file);
    }
  }
  return nieuw;
}

function ruimSessieOp(sessieDir) {
  try {
    fs.rmSync(sessieDir, { recursive: true, force: true });
  } catch {
    // laat staan; het is een tmp-map
  }
}

// ---- Assistent-run via `claude -p` --------------------------------------

async function handleAssistent(req, res) {
  let body = '';
  for await (const chunk of req) body += chunk;

  let payload;
  try {
    payload = JSON.parse(body);
  } catch {
    res.writeHead(400).end('ongeldige JSON');
    return;
  }
  const prompt = String(payload.prompt ?? '').trim();
  const modus = payload.modus === 'doel' ? 'doel' : 'instructie';
  if (!prompt) {
    res.writeHead(400).end('prompt ontbreekt');
    return;
  }
  // De werkversie uit de browser: [{ key: "id@valid_from", yaml }]. Die gaat
  // als beginstand in de overlays, zodat de assistent leest en rekent op wat
  // de gebruiker ziet (inclusief eigen bewerkingen en variant-bestanden).
  const documenten = Array.isArray(payload.documenten) ? payload.documenten : [];
  // Idem voor het uitvoeringslastmodel. Zonder dit leest de assistent altijd
  // de basis-handelingen.yaml, terwijl een variant er een eigen kan meebrengen
  // -- de po-accountant bestaat bijvoorbeeld alleen in nk-3-harmonisatie-po-vo.
  // Een instructie over die handeling liep dan dood op "bestaat niet".
  const handelingen = typeof payload.handelingen === 'string' ? payload.handelingen : null;

  res.writeHead(200, {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
  });
  const send = (event) => res.write(`data: ${JSON.stringify(event)}\n\n`);
  const heartbeat = setInterval(() => res.write(': ping\n\n'), 15000);

  const sessieDir = maakSessieMap();
  const beginstand = new Map();
  for (const d of documenten) {
    if (!d?.key || typeof d.yaml !== 'string') continue;
    fs.writeFileSync(path.join(sessieDir, 'overlays', `${encodeURIComponent(d.key)}.yaml`), d.yaml);
    beginstand.set(String(d.key), d.yaml);
  }
  // Beginstand van het uitvoeringslastmodel, zodat `klaar` straks alleen
  // meldt dat het gewijzigd is als het echt afwijkt van wat de browser stuurde.
  const handelingenBegin = handelingen;
  if (handelingen) fs.writeFileSync(path.join(sessieDir, 'handelingen.yaml'), handelingen);
  const gezienEvents = new Set();
  // Poll de MCP-event-map zodat wijziging/simulatie-events tijdens de run
  // binnenkomen, niet pas aan het eind.
  const eventPoll = setInterval(() => {
    for (const ev of leesNieuweEvents(sessieDir, gezienEvents)) send(ev);
  }, 300);

  const opdracht = modus === 'doel'
    ? `DOEL van de beleidsmakers: ${prompt}\n\nWerk iteratief naar dit doel toe zoals beschreven in je werkwijze.`
    : `INSTRUCTIE van de beleidsmakers: ${prompt}`;

  const mcpConfig = JSON.stringify({
    mcpServers: {
      regelrecht: {
        command: process.execPath,
        args: [MCP_SERVER],
        env: { OCW_SESSION_DIR: sessieDir },
      },
    },
  });

  // De unie over alle casus: een tool die een casus niet aanbiedt, staat
  // simpelweg niet in zijn tools/list en is dan onbereikbaar. Toestaan wat er
  // niet is kan dus geen kwaad; omgekeerd wel — een tool die de MCP-server
  // aanbiedt maar hier ontbreekt, weigert de CLI stil.
  const mcpTools = [
    'mcp__regelrecht__lees_regelgeving',
    'mcp__regelrecht__wijzig_definitie',
    'mcp__regelrecht__wijzig_regelgeving',
    'mcp__regelrecht__lees_handelingen',
    'mcp__regelrecht__wijzig_handelingen',
    'mcp__regelrecht__simuleer_personas',
    'mcp__regelrecht__simuleer_populatie',
  ];

  const args = [
    '-p', opdracht,
    '--output-format', 'stream-json',
    '--verbose',
    '--model', MODEL,
    '--system-prompt', SYSTEM,
    '--mcp-config', mcpConfig,
    '--strict-mcp-config',
    // Alleen de regelrecht-MCP-tools; alle ingebouwde tools (Bash/Read/Edit/
    // Agent/…) uit, zodat de assistent gesandboxed op de engine werkt en niet
    // in het echte bestandssysteem of andere sessies rommelt.
    '--tools', '',
    '--allowedTools', mcpTools.join(','),
    '--permission-mode', 'bypassPermissions',
    '--setting-sources', '',
    '--disable-slash-commands',
    // Doelmodus meet, stelt bij, meet opnieuw tot het convergeert en sluit af
    // met een eindmeting; dertig beurten was daar krap voor en kapte hem
    // midden in het kalibreren af.
    '--max-turns', modus === 'doel' ? '60' : '12',
  ];

  const child = spawn(CLAUDE_BIN, args, {
    cwd: sessieDir,
    stdio: ['ignore', 'pipe', 'pipe'],
    // De regelgevingsdocumenten zijn groot (de WSF 2000 is ~53 KB / ~20k
    // tokens). Zonder een ruime MCP-outputlimiet krijgt de assistent alleen een
    // preview van lees_regelgeving en kan hij de te wijzigen parameter niet
    // vinden. Ingebouwde tools staan uit, dus dit is zijn enige leeskanaal.
    env: { ...process.env, MAX_MCP_OUTPUT_TOKENS: '50000' },
  });

  let afgerond = false;
  const rond_af = (foutmelding) => {
    if (afgerond) return;
    afgerond = true;
    clearInterval(heartbeat);
    clearInterval(eventPoll);
    // Laatste events + eindstand van de overlays.
    for (const ev of leesNieuweEvents(sessieDir, gezienEvents)) send(ev);
    if (foutmelding) {
      send({ type: 'fout', melding: foutmelding });
    } else {
      // Alleen wat de assistent echt veranderde ten opzichte van de beginstand.
      const overlays = {};
      for (const [key, tekst] of Object.entries(leesOverlays(sessieDir))) {
        if (beginstand.get(key) !== tekst) overlays[key] = tekst;
      }
      // Het uitvoeringslastmodel apart: het gaat niet door de engine en wordt
      // in de app niet door de lawStore maar door useHandelingen opgepakt.
      // Alleen doorsturen als het echt afwijkt van de beginstand, anders zou
      // elke run een "wijziging" melden die er niet is.
      const handelingenEind = leesHandelingen(sessieDir);
      send({
        type: 'klaar',
        overlays,
        handelingen: handelingenEind !== handelingenBegin ? handelingenEind : null,
      });
    }
    ruimSessieOp(sessieDir);
    res.end();
  };

  // Kill het childproces als de browser de SSE-verbinding sluit.
  req.on('close', () => {
    if (!afgerond) {
      child.kill('SIGTERM');
      rond_af(null);
    }
  });

  let stdoutBuf = '';
  let stderrBuf = '';
  let laatsteResultaat = null;

  child.stdout.on('data', (chunk) => {
    stdoutBuf += chunk;
    let index;
    while ((index = stdoutBuf.indexOf('\n')) !== -1) {
      const line = stdoutBuf.slice(0, index).trim();
      stdoutBuf = stdoutBuf.slice(index + 1);
      if (line) {
        const res = verwerkStreamRegel(line, send);
        if (res) laatsteResultaat = res;
      }
    }
  });

  child.stderr.on('data', (chunk) => { stderrBuf += chunk; });

  child.on('error', (err) => {
    rond_af(`Kon de claude-CLI niet starten: ${String(err?.message ?? err)}`);
  });

  child.on('close', (code) => {
    if (code === 0 || code === null) {
      rond_af(null);
    } else {
      rond_af(foutmeldingVoor(code, stderrBuf, laatsteResultaat));
    }
  });
}

/**
 * Waarom stopte de assistent? De CLI zegt het zelf in de result-regel; alleen
 * als die ontbreekt vallen we terug op stderr of de exitcode.
 */
function foutmeldingVoor(code, stderrBuf, resultaat) {
  if (resultaat?.subtype === 'error_max_turns') {
    return `De assistent had meer stappen nodig dan de limiet van ${resultaat.num_turns ?? 'het maximum'}. `
      + 'Splits de opdracht op, of verhoog --max-turns in server/index.js.';
  }
  if (resultaat?.subtype === 'error_during_execution') {
    return 'De assistent liep vast tijdens het uitvoeren. Probeer de opdracht opnieuw of maak hem kleiner.';
  }
  if (stderrBuf.trim()) {
    return `De assistent stopte met een fout: ${stderrBuf.trim().split('\n').slice(-3).join(' ')}`;
  }
  return `De assistent stopte onverwacht (exitcode ${code}${resultaat?.subtype ? `, ${resultaat.subtype}` : ''}).`;
}

/**
 * Eén stream-json-regel van `claude -p` → SSE-events. Relevante types:
 *   system/init, rate_limit_event → negeren
 *   assistant → message.content[] met text- en tool_use-blokken
 *   result → einde (afhandeling gebeurt via child 'close')
 */
function verwerkStreamRegel(line, send) {
  let msg;
  try {
    msg = JSON.parse(line);
  } catch {
    return;
  }
  // De result-regel draagt de reden waarom de CLI stopt (bijvoorbeeld
  // error_max_turns). Zonder die regel eindigt het proces non-zero met lege
  // stderr en werd dat "exitcode 1", wat niets zegt.
  if (msg.type === 'result') {
    return { subtype: msg.subtype ?? null, is_error: !!msg.is_error, num_turns: msg.num_turns ?? null };
  }
  if (msg.type !== 'assistant' || !msg.message?.content) return;

  for (const block of msg.message.content) {
    if (block.type === 'text' && block.text?.trim()) {
      send({ type: 'tekst', tekst: block.text });
    } else if (block.type === 'tool_use') {
      const naam = String(block.name ?? '').replace(/^mcp__regelrecht__/, '');
      // De wijziging/simulatie-details komen als eigen events uit de MCP-server;
      // hier tonen we alleen dat de tool is aangeroepen (met de invoer, behalve
      // de volledige YAML van een wijziging).
      let input = block.input ?? {};
      if (naam === 'wijzig_regelgeving') {
        input = { document_key: input.document_key, toelichting: input.toelichting };
      }
      send({ type: 'tool', naam, input });
    }
  }
}

// ---- Bewaar als variant: git-branch vanaf main -------------------------

const CASUS_DIR = path.resolve(__dirname, '..');

function git(args, cwd) {
  return new Promise((resolve, reject) => {
    execFile('git', args, { cwd, maxBuffer: 16 * 1024 * 1024 }, (err, stdout, stderr) => {
      if (err) reject(new Error(String(stderr || err.message).trim()));
      else resolve(String(stdout).trim());
    });
  });
}

function runNode(script, cwd) {
  return new Promise((resolve, reject) => {
    execFile(process.execPath, [script], { cwd, maxBuffer: 16 * 1024 * 1024 }, (err, stdout, stderr) => {
      if (err) reject(new Error(String(stderr || err.message).trim()));
      else resolve(String(stdout).trim());
    });
  });
}

/**
 * POST /api/variant  { sleutel, titel, bestanden: [{ path, yaml }], basis? }
 *   → { ok, id, branch }
 *
 * Maakt in een tijdelijke worktree de branch variant/<sleutel> vanaf main,
 * schrijft de bestanden (paden relatief aan de casusmap, bijvoorbeeld
 * corpus/regulation/nl/…/2028-01-01.yaml), commit "Variant <sleutel>: <titel>"
 * en draait copy-assets zodat de app de variant meteen als kolom ziet. De
 * werkkopie van de gebruiker blijft onaangeroerd; de branch is lokaal tot hij
 * gepusht wordt.
 */
async function handleVariant(req, res) {
  let body = '';
  for await (const chunk of req) body += chunk;
  let payload;
  try {
    payload = JSON.parse(body);
  } catch {
    res.writeHead(400, { 'Content-Type': 'application/json' }).end(JSON.stringify({ fout: 'ongeldige JSON' }));
    return;
  }
  const antwoord = (status, obj) => {
    res.writeHead(status, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(obj));
  };

  const id = String(payload.sleutel ?? '').trim();
  const titel = String(payload.titel ?? '').trim().replace(/\s+/g, ' ');
  const bestanden = Array.isArray(payload.bestanden) ? payload.bestanden : [];
  if (!/^[a-z0-9][a-z0-9-]{1,40}$/.test(id)) return antwoord(400, { fout: 'Sleutel: kleine letters, cijfers en streepjes, 2 tot 41 tekens.' });
  if (!titel) return antwoord(400, { fout: 'Titel ontbreekt.' });
  if (!bestanden.length) return antwoord(400, { fout: 'Geen bewerkte bestanden om te bewaren.' });
  for (const b of bestanden) {
    const rel = String(b?.path ?? '');
    if (!rel.startsWith('corpus/') || rel.includes('..') || !rel.endsWith('.yaml') || typeof b.yaml !== 'string') {
      return antwoord(400, { fout: `Ongeldig bestandspad: ${rel}` });
    }
  }
  const branch = `variant/${id}`;

  let repoRoot;
  try {
    repoRoot = await git(['rev-parse', '--show-toplevel'], CASUS_DIR);
  } catch (e) {
    return antwoord(500, { fout: `Geen git-repository gevonden: ${e.message}` });
  }
  const casusRel = path.relative(repoRoot, CASUS_DIR);
  try {
    await git(['rev-parse', '--verify', '--quiet', `refs/heads/${branch}`], repoRoot);
    return antwoord(409, { fout: `Branch ${branch} bestaat al; kies een andere sleutel.` });
  } catch {
    // bestaat niet: goed
  }

  const worktree = fs.mkdtempSync(path.join(os.tmpdir(), 'ocw-variant-'));
  try {
    fs.rmSync(worktree, { recursive: true, force: true });
    await git(['worktree', 'add', '--quiet', worktree, '-b', branch, 'main'], repoRoot);
    const toegevoegd = [];
    for (const b of bestanden) {
      const doel = path.join(worktree, casusRel, b.path);
      fs.mkdirSync(path.dirname(doel), { recursive: true });
      fs.writeFileSync(doel, b.yaml);
      toegevoegd.push(path.join(casusRel, b.path));
    }
    await git(['add', '--', ...toegevoegd], worktree);
    const toelichting = payload.basis
      ? `Bewaard vanuit de werkversie ${payload.basis} in de beleidsomgeving.`
      : 'Bewaard vanuit de beleidsomgeving (huidig recht met bewerkingen).';
    // --no-verify: de pre-commit hook van deze repo draait `pre-commit run
    // --all-files` en toetst dus de hele werkboom, niet de staged bestanden.
    // In deze tijdelijke worktree staat alleen de gewijzigde corpus-YAML, dus
    // die run zou struikelen over bestanden die niets met deze variant te
    // maken hebben. De inhoud is bovendien al gevalideerd (validateYaml)
    // voordat we hier komen.
    await git(['-c', 'user.name=beleidsomgeving', '-c', 'user.email=beleidsomgeving@localhost', 'commit', '--quiet', '--no-verify', '-m', `Variant ${id}: ${titel}`, '-m', toelichting], worktree);
  } catch (e) {
    try { await git(['worktree', 'remove', '--force', worktree], repoRoot); } catch { /* al weg */ }
    try { await git(['branch', '-D', branch], repoRoot); } catch { /* niet aangemaakt */ }
    return antwoord(500, { fout: `Branch maken mislukt: ${e.message}` });
  }
  try {
    await git(['worktree', 'remove', '--force', worktree], repoRoot);
  } catch (e) {
    console.warn('worktree opruimen mislukt:', e.message);
  }
  try {
    await runNode(path.join(CASUS_DIR, 'app', 'scripts', 'copy-assets.js'), CASUS_DIR);
    // Vite's public-cache moet de nieuwe bestanden nog zien (bestandswatcher).
    await new Promise((r) => setTimeout(r, 400));
  } catch (e) {
    return antwoord(500, { fout: `Branch ${branch} is gemaakt, maar de export naar de app mislukte: ${e.message}` });
  }
  antwoord(200, { ok: true, id, branch });
}

// ---- HTTP-server --------------------------------------------------------

const server = http.createServer((req, res) => {
  if (req.method === 'POST' && req.url === '/api/assistent') {
    handleAssistent(req, res).catch((e) => {
      console.error(e);
      if (!res.headersSent) res.writeHead(500);
      res.end();
    });
    return;
  }
  if (req.method === 'POST' && req.url === '/api/variant') {
    if (!VARIANT_OPSLAG) {
      // 410 en niet 404: het endpoint bestaat, het kan hier alleen niet. Een
      // variant bewaren maakt een git-branch in de casus-checkout, en die is er
      // in de container niet. Varianten komen hosted uit ingecheckte bestanden
      // onder corpus-poc/<casus>/varianten/.
      res.writeHead(410, { 'Content-Type': 'application/json' });
      res.end(
        JSON.stringify({
          fout:
            'Een variant bewaren kan alleen lokaal: hosted staat er geen ' +
            'schrijfbare checkout onder deze demo. Varianten komen hier uit ' +
            'de repo (corpus-poc/<casus>/varianten/).',
        }),
      );
      return;
    }
    handleVariant(req, res).catch((e) => {
      console.error(e);
      if (!res.headersSent) res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ fout: String(e?.message ?? e) }));
    });
    return;
  }
  if (req.method === 'GET' && req.url === '/api/health') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    // `varianten` zegt of "Bewaar als variant" hier iets kan; de balk in de
    // app verbergt de knop erop, zodat niemand op een 410 klikt.
    res.end(
      JSON.stringify({
        ok: true,
        model: MODEL,
        cli: claudeCliOk === true,
        casus: CASUS,
        varianten: VARIANT_OPSLAG,
      }),
    );
    return;
  }
  res.writeHead(404).end();
});

checkClaudeCli().then((ok) => {
  server.listen(PORT, () => {
    const status = ok ? `claude-CLI gevonden (${CLAUDE_BIN})` : `LET OP: claude-CLI niet gevonden (${CLAUDE_BIN})`;
    console.log(`beleidsassistent draait op http://localhost:${PORT} (model ${MODEL}) — ${status}`);
  });
});
