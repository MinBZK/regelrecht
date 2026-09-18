/**
 * Stdio-MCP-server met de vier beleidsassistent-tools (lees/wijzig regelgeving,
 * simuleer persona's/populatie). Draait als childproces van `claude -p`; de
 * CLI praat er via JSON-RPC 2.0 over stdin/stdout mee (newline-gescheiden).
 *
 * State-probleem: dit proces is een kind van claude, niet van het HTTP-proces,
 * dus de overlays (bewerkte YAML per document) kunnen niet in het HTTP-proces
 * leven. Oplossing: elke sessie krijgt een tijdelijke sessiemap (pad via env
 * OCW_SESSION_DIR). Overlays worden daarheen weggeschreven en voortgang wordt
 * als losse event-JSON-bestanden neergelegd; het HTTP-proces watcht die map en
 * vertaalt ze naar SSE. Zo blijft de bestaande engine/validatie-logica intact.
 */
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from 'fs';
import { pathToFileURL } from 'url';
import { resolve } from 'path';
import {
  appSrc,
  createEngine,
  corpusMetOverlays,
  loadDistributions,
  loadPersonas,
  validateYaml,
} from '../engine.js';

// Per casus een eigen app-map, dus dynamisch; zie nieuwkomersbekostiging.mcp.js.
const mod = (rel) => import(pathToFileURL(resolve(appSrc, rel)).href);
const { simulate } = await mod('sim/simulate.js');
const { generatePopulation } = await mod('sim/population.js');
const { aggregate } = await mod('sim/metrics.js');
const { patchDefinitionValue, readDefinitionValue, patchArticleText, readArticleText } = await mod('lib/yamlPatch.js');

const SIM_OPTIONS = { startJaar: 2026 };
const SESSION_DIR = process.env.OCW_SESSION_DIR;

if (!SESSION_DIR) {
  process.stderr.write('OCW_SESSION_DIR ontbreekt\n');
  process.exit(1);
}
mkdirSync(SESSION_DIR, { recursive: true });

const overlaysDir = resolve(SESSION_DIR, 'overlays');
const eventsDir = resolve(SESSION_DIR, 'events');
mkdirSync(overlaysDir, { recursive: true });
mkdirSync(eventsDir, { recursive: true });

let eventSeq = 0;

function euro(cents) {
  return (cents / 100).toLocaleString('nl-NL', { style: 'currency', currency: 'EUR' });
}

/** Overlays als map key → yaml-tekst, herladen van schijf per aanroep. */
function readOverlays() {
  const overlays = new Map();
  if (!existsSync(overlaysDir)) return overlays;
  for (const file of readdirSync(overlaysDir)) {
    if (!file.endsWith('.yaml')) continue;
    const key = decodeURIComponent(file.slice(0, -'.yaml'.length));
    overlays.set(key, readFileSync(resolve(overlaysDir, file), 'utf-8'));
  }
  return overlays;
}

function writeOverlay(key, yaml) {
  writeFileSync(resolve(overlaysDir, `${encodeURIComponent(key)}.yaml`), yaml);
}

/** Leg een voortgangs-event neer voor het HTTP-proces (oplopend genummerd). */
function emit(event) {
  const seq = String(++eventSeq).padStart(6, '0');
  writeFileSync(resolve(eventsDir, `${seq}-${process.pid}.json`), JSON.stringify(event));
}

/**
 * Wat de assistent tot nu toe heeft gewijzigd, als `artikel::naam` → waarde.
 *
 * Hangt aan elke meting mee, zodat de grafiek in de app kan laten zien wélke
 * stand bij een punt hoort. Zonder dit is het pad een reeks uitkomsten zonder
 * oorzaak: je ziet dat het beter wordt, niet waardoor, en je kunt er dus ook
 * geen tussenstand uit overnemen.
 *
 * Alleen de gewijzigde definities en niet de hele YAML: die is hier ~53 KB, en
 * een doel-run doet twintig metingen. De app heeft aan de waarden genoeg om de
 * stand te tonen en de wijziging over te nemen.
 */
const gewijzigdeDefinities = new Map();

/**
 * Documenten die met wijzig_regelgeving zijn herschreven. Daarvan is niet uit
 * losse waarden te zeggen wat er veranderd is, dus zegt de app daar "structuur
 * gewijzigd" in plaats van een onvolledige lijst.
 */
const structuurGewijzigd = new Set();

function onthoudWijziging(document_key, artikel, naam, oud, nieuw) {
  gewijzigdeDefinities.set(`${document_key}::${artikel}::${naam}`, {
    document_key, artikel: String(artikel), naam, oud, nieuw,
  });
}

/** De stand van de wijzigingen nu, als lijst voor een meting-event. */
function huidigeWijzigingen() {
  return [...gewijzigdeDefinities.values()];
}

async function simulateRecords(records, defaultChoices = {}) {
  const overlays = readOverlays();
  const engine = await createEngine(overlays);
  const results = [];
  for (const record of records) {
    const choices = { ...(record.keuzes ?? {}), ...defaultChoices };
    try {
      const { totals } = simulate(engine, record, choices, SIM_OPTIONS);
      results.push({ record, totals });
    } catch (e) {
      results.push({ record, error: String(e?.message ?? e) });
    }
    // De engine cachet resolutie per databron-sleutel; zonder reset per record
    // loopt de doorlooptijd lineair op (zie simWorker.js).
    engine.clearDataSources?.();
  }
  engine.free?.();
  return results;
}

// ---- Toolimplementaties -------------------------------------------------

// Ruime bovengrens: onder deze lengte geven we de hele YAML in één keer terug.
// Grotere documenten (zoals de WSF 2000, ~53 KB) worden in regelvensters
// gelezen, zodat een enkele tool-output de outputlimiet niet overschrijdt.
const HELE_DOC_MAX = 12000;

async function leesRegelgeving({ document_key, offset, limit, zoek }) {
  // De werkversie uit de browser staat als overlays klaar; nieuwe versiebestanden
  // van een variant (zonder basisdocument) horen daar ook bij.
  const corpus = corpusMetOverlays(readOverlays());
  if (!document_key) {
    return JSON.stringify(
      corpus.map((d) => ({ key: d.key, naam: d.name, geldig_vanaf: d.valid_from, gewijzigd: !!d.gewijzigd })),
    );
  }
  const doc = corpus.find((d) => d.key === document_key);
  if (!doc) return `Onbekend document: ${document_key}`;
  const yaml = doc.yaml;
  const lines = yaml.split('\n');

  // zoek: geef genummerde regels terug die op de (regex-)term matchen, zodat de
  // assistent een parameter kan lokaliseren zonder de hele wet in te lezen.
  if (zoek) {
    let re;
    try {
      re = new RegExp(zoek, 'i');
    } catch {
      re = new RegExp(zoek.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'i');
    }
    const treffers = lines
      .map((tekst, i) => ({ nr: i + 1, tekst }))
      .filter(({ tekst }) => re.test(tekst));
    if (!treffers.length) return `Geen regels gevonden voor /${zoek}/i in ${document_key}.`;
    return treffers.map(({ nr, tekst }) => `${nr}: ${tekst}`).join('\n');
  }

  // Genummerde regels (net als een editor), zodat offset/limit en een latere
  // wijziging op dezelfde nummering leunen. offset is 1-gebaseerd.
  const genummerd = (van, tot) =>
    lines.slice(van, tot).map((tekst, i) => `${van + i + 1}: ${tekst}`).join('\n');

  if (offset != null || limit != null) {
    const van = Math.max(0, (offset ?? 1) - 1);
    const tot = limit != null ? van + limit : lines.length;
    return `${genummerd(van, Math.min(tot, lines.length))}\n[regels ${van + 1}-${Math.min(tot, lines.length)} van ${lines.length}]`;
  }

  if (yaml.length > HELE_DOC_MAX) {
    return (
      `Dit document is groot (${lines.length} regels). Lees het in vensters met ` +
      `offset/limit, of gebruik "zoek" om een parameter te lokaliseren. Hieronder ` +
      `de eerste 200 regels:\n\n${genummerd(0, 200)}\n[regels 1-200 van ${lines.length}]`
    );
  }
  return yaml;
}

async function wijzigRegelgeving({ document_key, nieuwe_yaml, toelichting }) {
  const overlays = readOverlays();
  const result = await validateYaml(overlays, document_key, nieuwe_yaml);
  if (!result.ok) {
    return `Validatie mislukt, wijziging NIET toegepast: ${result.error}`;
  }
  writeOverlay(document_key, nieuwe_yaml);
  // Een structuurwijziging herschrijft het hele document, dus wat we van de
  // losse definities bijhielden klopt daarna niet meer. Vergeten is hier
  // eerlijker dan een lijst tonen die niet meer zegt wat er is veranderd.
  structuurGewijzigd.add(document_key);
  emit({ type: 'wijziging', document_key, toelichting: toelichting ?? '' });
  return 'Wijziging gevalideerd en toegepast.';
}

/**
 * Wijzig één definitie-waarde in een document (tekstpatch van één regel):
 * de snelle weg voor percentages, voeten, bedragen en termijnen.
 */
async function wijzigDefinitie({ document_key, artikel, naam, waarde, nieuwe_wettekst, toelichting }) {
  const overlays = readOverlays();
  const doc = corpusMetOverlays(overlays).find((d) => d.key === document_key);
  if (!doc) return `Onbekend document: ${document_key}`;
  const huidig = doc.yaml;
  let oud;
  let nieuw;
  try {
    oud = readDefinitionValue(huidig, String(artikel), naam);
    nieuw = patchDefinitionValue(huidig, String(artikel), naam, Number(waarde));
  } catch (e) {
    return `Definitie niet gevonden: ${e.message}. Zoek de naam met lees_regelgeving (zoek=...) en let op het artikelnummer als string, bijvoorbeeld "10a.8".`;
  }
  // De wettekst mee, als de assistent hem meestuurt. Een wet is zijn tekst:
  // alleen de waarde patchen laat de proza de oude regel vertellen, en in een
  // demo over wetgeving is dat precies de verkeerde indruk.
  let tekstGewijzigd = false;
  if (typeof nieuwe_wettekst === 'string' && nieuwe_wettekst.trim()) {
    try {
      const oudeTekst = readArticleText(nieuw, String(artikel));
      nieuw = patchArticleText(nieuw, String(artikel), nieuwe_wettekst.trim());
      tekstGewijzigd = oudeTekst !== readArticleText(nieuw, String(artikel));
    } catch (e) {
      return `De waarde is NIET gewijzigd omdat de wettekst niet kon worden bijgewerkt: ${e.message}`;
    }
  }
  const result = await validateYaml(overlays, document_key, nieuw);
  if (!result.ok) return `Validatie mislukt, wijziging NIET toegepast: ${result.error}`;
  writeOverlay(document_key, nieuw);
  onthoudWijziging(document_key, artikel, naam, oud, Number(waarde));
  const staart = tekstGewijzigd ? ' De wettekst van dit artikel is meegeschreven.' : '';
  emit({
    type: 'wijziging',
    document_key,
    toelichting: (toelichting ?? `${naam} in artikel ${artikel}: ${oud} -> ${waarde}`) + staart,
    wettekst_gewijzigd: tekstGewijzigd,
  });
  return `Toegepast: ${naam} in artikel ${artikel} van ${oud} naar ${waarde} (gevalideerd).${staart}`
    + (tekstGewijzigd ? '' : ' LET OP: de wettekst van dit artikel vertelt nu nog de oude regel.'
      + ' Stuur nieuwe_wettekst mee als de tekst het gewijzigde getal noemt.');
}

async function simuleerPersonas() {
  const personas = loadPersonas();
  const results = await simulateRecords(
    personas.map((p) => ({
      ...p,
      keuzes: {
        draagkrachtAangevraagd: p.keuzemomenten?.includes('draagkrachtmeting') ?? false,
        partnerMeetellen: true,
      },
    })),
  );
  emit({ type: 'simulatie', doel: 'personas' });
  return JSON.stringify(
    results.map(({ record, totals, error }) =>
      error
        ? { naam: record.naam, error }
        : {
            naam: record.naam,
            regime: totals.regime,
            maxMaandbedrag: euro(totals.maxMaandbedrag),
            totaalBetaald: euro(totals.totaalBetaald),
            kwijtgescholden: euro(totals.kwijtgescholden),
            maanden: totals.maanden,
            levenslang: totals.levenslang,
            jarenBetalingsprobleem: totals.jarenBetalingsprobleem,
          },
    ),
  );
}

async function simuleerPopulatie({ n, seed }) {
  const distributions = loadDistributions();
  if (!distributions) {
    return 'Geen data/distributions.yaml gevonden; gebruik simuleer_personas.';
  }
  const count = Math.max(50, Math.min(n ?? 300, 2000));
  const records = generatePopulation(distributions, count, seed ?? 42);
  const results = await simulateRecords(records);
  const ok = results.filter((r) => !r.error);
  const metrics = aggregate(ok);
  emit({
    type: 'simulatie',
    doel: 'populatie',
    n: count,
    metrics,
    // De stand waarop deze meting rust, zodat een punt in het optimalisatiepad
    // te openen en over te nemen is.
    wijzigingen: huidigeWijzigingen(),
    structuurGewijzigd: [...structuurGewijzigd],
  });
  return JSON.stringify({
    n: count,
    fouten: results.length - ok.length,
    metrics: { ...metrics, kwijtgescholdenTotaal: euro(metrics.kwijtgescholdenTotaal) },
  });
}

// ---- Toolregister + JSON-schema's --------------------------------------

const TOOLS = [
  {
    name: 'lees_regelgeving',
    description:
      'Lees de actuele machine-uitvoerbare YAML van een regelgevingsdocument, ' +
      'inclusief eerdere wijzigingen in deze sessie. Zonder document_key: ' +
      'geeft de lijst van beschikbare documenten. Grote documenten (zoals de ' +
      'Wet studiefinanciering 2000) worden niet volledig teruggegeven; gebruik ' +
      '"zoek" om een parameter te lokaliseren en dan offset/limit om het ' +
      'relevante venster genummerd te lezen. De regelnummers in de uitvoer ' +
      'komen overeen met de bronregels.',
    inputSchema: {
      type: 'object',
      properties: {
        document_key: {
          type: 'string',
          description: 'Documentsleutel, bv. "wet_studiefinanciering_2000@2025-01-01"',
        },
        zoek: {
          type: 'string',
          description:
            'Regex (hoofdletterongevoelig); geeft alle matchende regels met ' +
            'regelnummer terug, zodat je een parameter kunt vinden zonder de ' +
            'hele wet te lezen.',
        },
        offset: {
          type: 'integer',
          description: '1-gebaseerd regelnummer waar het leesvenster begint.',
        },
        limit: {
          type: 'integer',
          description: 'Aantal regels vanaf offset.',
        },
      },
      additionalProperties: false,
    },
    run: leesRegelgeving,
  },
  {
    name: 'wijzig_regelgeving',
    description:
      'Vervang de VOLLEDIGE YAML van een regelgevingsdocument (niet enkel het ' +
      'gewijzigde fragment). Bij een groot document: lees het eerst helemaal in ' +
      'met offset/limit-vensters, pas het doelfragment aan, en stuur de complete ' +
      'YAML terug. De nieuwe versie wordt eerst door de regelrecht-engine ' +
      'gevalideerd; bij een fout blijft de oude versie gelden en krijg je de ' +
      'foutmelding terug. Wijzig waarden via definitions en structuur via de ' +
      'operation trees; houd artikelnummers en wettekst-referenties intact.',
    inputSchema: {
      type: 'object',
      properties: {
        document_key: { type: 'string' },
        nieuwe_yaml: { type: 'string' },
        toelichting: {
          type: 'string',
          description: 'Korte omschrijving van de wijziging, voor de gebruiker',
        },
      },
      required: ['document_key', 'nieuwe_yaml'],
      additionalProperties: false,
    },
    run: wijzigRegelgeving,
  },
  {
    name: 'wijzig_definitie',
    description:
      'Wijzig één waarde in de definitions van een artikel: percentages en ' +
      'ratio\'s (als getal, 0.84 voor 84%), bedragen in eurocent, termijnen in ' +
      'maanden. Dit is de snelle en veilige weg voor parameterwijzigingen: één ' +
      'aanroep, geen document herschrijven. Vind eerst de naam en het artikel ' +
      'met lees_regelgeving (zoek=...); het artikelnummer is een string zoals ' +
      '"10a.8" of "6.10". De nieuwe waarde wordt door de engine gevalideerd.',
    inputSchema: {
      type: 'object',
      properties: {
        document_key: { type: 'string', description: 'bv. "wet_studiefinanciering_2000@2025-01-01"' },
        artikel: { type: 'string', description: 'artikelnummer zoals in de YAML, bv. "10a.8"' },
        naam: { type: 'string', description: 'naam van de definitie, bv. "voet_ratio_overig_sf15_oud"' },
        waarde: { type: 'number', description: 'nieuwe waarde (ratio als getal, eurocent voor bedragen)' },
        nieuwe_wettekst: {
          type: 'string',
          description:
            'De wettekst van dit artikel, herschreven zodat hij de nieuwe waarde vertelt. '
            + 'Stuur dit mee zodra de tekst het gewijzigde getal noemt: een wet is zijn tekst, '
            + 'en alleen de waarde aanpassen laat de proza de oude regel vertellen. Schrijf in de '
            + 'stijl van het artikel zelf, wijzig alleen wat de wijziging raakt, en laat '
            + 'artikelnummers en verwijzingen staan. Weglaten als de tekst het getal niet noemt.',
        },
        toelichting: { type: 'string', description: 'Korte omschrijving van de wijziging, voor de gebruiker' },
      },
      required: ['document_key', 'artikel', 'naam', 'waarde'],
      additionalProperties: false,
    },
    run: wijzigDefinitie,
  },
  {
    name: 'simuleer_personas',
    description:
      "Simuleer de terugbetaling voor de zeven demo-persona's onder de " +
      'actuele regelgeving (inclusief jouw wijzigingen). Geeft per persona ' +
      'regime, maandbedrag, kwijtschelding, looptijd en betalingsproblemen.',
    inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    run: simuleerPersonas,
  },
  {
    name: 'simuleer_populatie',
    description:
      'Simuleer een synthetische populatie debiteuren onder de actuele ' +
      'regelgeving en geef gewogen metrics terug: aantal en percentage met ' +
      'betalingsproblemen, gemiddeld maandbedrag per regime, totale ' +
      'kwijtschelding (kosten-proxy) en aantal levenslang-debiteuren. ' +
      'Gebruik een bescheiden n (standaard 300) in een optimalisatielus; ' +
      'maximaal 2000 voor een eindmeting.',
    inputSchema: {
      type: 'object',
      properties: {
        n: { type: 'integer', description: 'Aantal synthetische records (50-2000)' },
        seed: { type: 'integer' },
      },
      additionalProperties: false,
    },
    run: simuleerPopulatie,
  },
];

const TOOLS_BY_NAME = new Map(TOOLS.map((t) => [t.name, t]));

// ---- JSON-RPC 2.0 over stdio (minimale MCP-implementatie) --------------

const SERVER_INFO = { name: 'regelrecht', version: '0.1.0' };
const PROTOCOL_VERSION = '2024-11-05';

function send(message) {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

function reply(id, result) {
  send({ jsonrpc: '2.0', id, result });
}

function replyError(id, code, message) {
  send({ jsonrpc: '2.0', id, error: { code, message } });
}

async function handleMessage(msg) {
  const { id, method, params } = msg;

  if (method === 'initialize') {
    reply(id, {
      protocolVersion: PROTOCOL_VERSION,
      capabilities: { tools: {} },
      serverInfo: SERVER_INFO,
    });
    return;
  }
  if (method === 'notifications/initialized') return; // notificatie, geen antwoord
  if (method === 'ping') {
    reply(id, {});
    return;
  }
  if (method === 'tools/list') {
    reply(id, {
      tools: TOOLS.map((t) => ({
        name: t.name,
        description: t.description,
        inputSchema: t.inputSchema,
      })),
    });
    return;
  }
  if (method === 'tools/call') {
    const tool = TOOLS_BY_NAME.get(params?.name);
    if (!tool) {
      replyError(id, -32602, `Onbekende tool: ${params?.name}`);
      return;
    }
    try {
      const text = await tool.run(params.arguments ?? {});
      reply(id, { content: [{ type: 'text', text: String(text) }] });
    } catch (e) {
      reply(id, {
        content: [{ type: 'text', text: `Fout: ${String(e?.message ?? e)}` }],
        isError: true,
      });
    }
    return;
  }

  if (id !== undefined) replyError(id, -32601, `Onbekende methode: ${method}`);
}

let buffer = '';
process.stdin.on('data', (chunk) => {
  buffer += chunk;
  let index;
  while ((index = buffer.indexOf('\n')) !== -1) {
    const line = buffer.slice(0, index).trim();
    buffer = buffer.slice(index + 1);
    if (!line) continue;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      continue; // ongeldige regel overslaan
    }
    handleMessage(msg).catch((e) => process.stderr.write(`${String(e)}\n`));
  }
});

process.stdin.on('end', () => process.exit(0));
