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
import { load as yamlLoad } from 'js-yaml';
import {
  appSrc,
  createEngine,
  corpusMetOverlays,
  loadDistributions,
  loadHandelingen,
  loadPersonas,
  readHandelingenYaml,
  validateYaml,
} from '../engine.js';

// De gedeelde app-modules liggen per casus (frontend-poc-<casus>/src), dus het
// pad is pas op runtime bekend en de import moet dynamisch. Statisch importeren
// wees naar een map die in deze repo niet bestaat, waardoor deze hele MCP-server
// niet laadde en elke tool onbereikbaar was.
const mod = (rel) => import(pathToFileURL(resolve(appSrc, rel)).href);
const { simulate, evaluateLeerlingTimeline } = await mod('sim/simulate.js');
const { generatePopulation } = await mod('sim/population.js');
const { aggregate } = await mod('sim/metrics.js');
const { patchDefinitionValue, readDefinitionValue } = await mod('lib/yamlPatch.js');
const { DEFAULT_JAREN, peildataTussen, PERSONA_PEILDATA_VAN, PERSONA_PEILDATA_TOT, categorieLabel } =
  await mod('lib/nieuwkomerFacts.js');
const {
  ACTIES,
  addHandeling,
  keurHandelingenArgumenten,
  listHandelingen,
  listTarieven,
  patchHandeling,
  patchTarief,
  removeHandeling,
} = await mod('lib/handelingenPatch.js');

const SESSION_DIR = process.env.OCW_SESSION_DIR;
const PERSONA_PEILDATA = peildataTussen(PERSONA_PEILDATA_VAN, PERSONA_PEILDATA_TOT);

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
  return (cents / 100).toLocaleString('nl-NL', { style: 'currency', currency: 'EUR', maximumFractionDigits: 0 });
}

function mln(cents) {
  return `${(cents / 1e8).toLocaleString('nl-NL', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} mln`;
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

// Het uitvoeringslastmodel is geen wet: het gaat niet door de engine en hoort
// dus niet tussen de corpus-overlays, die het HTTP-proces als regelgeving
// uitlevert. Eigen bestand naast overlays/, met een eigen document_key in het
// wijziging-event zodat de UI het als aparte wijziging toont.
const HANDELINGEN_KEY = 'data/handelingen.yaml';
const handelingenOverlay = resolve(SESSION_DIR, 'handelingen.yaml');

/** De werkversie van het uitvoeringslastmodel: overlay als die er is, anders de basis. */
function readHandelingenWerkversie() {
  if (existsSync(handelingenOverlay)) return readFileSync(handelingenOverlay, 'utf-8');
  return readHandelingenYaml();
}

function writeHandelingenWerkversie(yaml) {
  writeFileSync(handelingenOverlay, yaml);
}

/** Leg een voortgangs-event neer voor het HTTP-proces (oplopend genummerd). */
function emit(event) {
  const seq = String(++eventSeq).padStart(6, '0');
  writeFileSync(resolve(eventsDir, `${seq}-${process.pid}.json`), JSON.stringify(event));
}

/**
 * Wat de assistent tot nu toe heeft gewijzigd, als `document::artikel::naam`.
 *
 * Hangt aan elke meting mee, zodat de grafiek in de app kan laten zien welke
 * stand bij een punt hoort. Zonder dit is het pad een reeks uitkomsten zonder
 * oorzaak: je ziet dat het beter wordt, niet waardoor, en je kunt er dus ook
 * geen tussenstand uit overnemen.
 *
 * Alleen de gewijzigde definities en niet de hele YAML: die documenten zijn
 * groot en een doel-run doet twintig metingen.
 */
const gewijzigdeDefinities = new Map();

/**
 * Documenten die met wijzig_regelgeving zijn herschreven, plus een vlag voor
 * het uitvoeringslastmodel. Daarvan is niet uit losse waarden te zeggen wat er
 * veranderd is, dus meldt de app dat in plaats van een onvolledige lijst.
 */
const structuurGewijzigd = new Set();
let handelingenGewijzigd = false;

function onthoudWijziging(document_key, artikel, naam, oud, nieuw) {
  gewijzigdeDefinities.set(`${document_key}::${artikel}::${naam}`, {
    document_key, artikel: String(artikel), naam, oud, nieuw,
  });
}

/** De stand van de wijzigingen nu, als lijst voor een meting-event. */
function huidigeWijzigingen() {
  return [...gewijzigdeDefinities.values()];
}

// ---- Toolimplementaties -------------------------------------------------

// Ruime bovengrens: onder deze lengte geven we de hele YAML in één keer terug.
// Grotere documenten worden in regelvensters gelezen, zodat een enkele
// tool-output de outputlimiet niet overschrijdt.
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
  // assistent een parameter kan lokaliseren zonder de hele regeling in te lezen.
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
 * de snelle weg voor bedragen, kwartalen, drempels en termijnen.
 */
async function wijzigDefinitie({ document_key, artikel, naam, waarde, toelichting }) {
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
    return `Definitie niet gevonden: ${e.message}. Zoek de naam met lees_regelgeving (zoek=...) en let op het artikelnummer als string, bijvoorbeeld "34, tweede lid".`;
  }
  const result = await validateYaml(overlays, document_key, nieuw);
  if (!result.ok) return `Validatie mislukt, wijziging NIET toegepast: ${result.error}`;
  writeOverlay(document_key, nieuw);
  onthoudWijziging(document_key, artikel, naam, oud, Number(waarde));
  emit({ type: 'wijziging', document_key, toelichting: toelichting ?? `${naam} in artikel ${artikel}: ${oud} -> ${waarde}` });
  return `Toegepast: ${naam} in artikel ${artikel} van ${oud} naar ${waarde} (gevalideerd).`;
}

// ---- Uitvoeringslastmodel (data/handelingen.yaml) -----------------------

/**
 * Lees het uitvoeringslastmodel: de handelingen met hun minuten en tarief, de
 * tarieven zelf. Dit staat naast de regelgeving: een handeling is wat de wet
 * in de praktijk aan werk kost, niet wat er in de wet staat.
 */
async function leesHandelingen() {
  const tekst = readHandelingenWerkversie();
  if (!tekst) return 'Deze casus heeft geen data/handelingen.yaml.';
  return JSON.stringify({
    tarieven: listTarieven(tekst),
    handelingen: listHandelingen(tekst),
  });
}

/**
 * Wijzig het uitvoeringslastmodel. Eén tool met een actie, in plaats van drie
 * losse: toevoegen, verwijderen en aanpassen delen hun doelbestand, hun
 * validatie en hun wijziging-event, en de assistent hoeft zo maar één tool te
 * kennen om "geen accountant in het po" uit te voeren.
 */
async function wijzigHandelingen({ actie, id, velden, handeling, tarief, waarde, toelichting }) {
  const huidig = readHandelingenWerkversie();
  if (!huidig) return 'Deze casus heeft geen data/handelingen.yaml.';

  // Het JSON-schema noemt alleen `actie` verplicht, omdat "verplicht bij deze
  // actie" er alleen als oneOf in past. Deze controle zegt in één melding welk
  // argument bij welke actie hoort, in plaats van de patch-functie te laten
  // struikelen over een waarde die er nooit was.
  const argumentFout = keurHandelingenArgumenten({ actie, id, velden, handeling, tarief, waarde });
  if (argumentFout) return `Wijziging NIET toegepast: ${argumentFout}`;

  let nieuw;
  let melding;
  try {
    if (actie === 'verwijder') {
      const r = removeHandeling(huidig, id);
      nieuw = r.yaml;
      melding = `Handeling ${id} verwijderd (${r.verwijderd.partij}, ${r.verwijderd.sector ?? 'po en vo'}, ${r.verwijderd.minuten} min).`;
    } else if (actie === 'voeg_toe') {
      const r = addHandeling(huidig, handeling);
      nieuw = r.yaml;
      melding = `Handeling ${r.toegevoegd.id} toegevoegd (${r.toegevoegd.minuten} min, tarief ${r.toegevoegd.tarief}).`;
    } else if (actie === 'wijzig') {
      const r = patchHandeling(huidig, id, velden);
      nieuw = r.yaml;
      melding = `Handeling ${id} gewijzigd: ${r.veranderd.join('; ')}.`;
    } else {
      const r = patchTarief(huidig, tarief, waarde);
      nieuw = r.yaml;
      melding = `Tarief ${tarief} van ${r.oud} naar ${r.nieuw} eurocent per uur.`;
    }
  } catch (e) {
    return `Wijziging NIET toegepast: ${String(e?.message ?? e)}`;
  }

  writeHandelingenWerkversie(nieuw);
  // Het uitvoeringslastmodel is geen wet en gaat niet door de definitie-lijst;
  // een punt in het pad meldt alleen dát het is bijgesteld.
  handelingenGewijzigd = true;
  emit({ type: 'wijziging', document_key: HANDELINGEN_KEY, toelichting: toelichting || melding });
  return `${melding} Reken het effect door met simuleer_populatie.`;
}

async function simuleerPersonas() {
  const personas = loadPersonas();
  if (!personas.length) return 'Geen data/personas.yaml gevonden.';
  const engine = await createEngine(readOverlays());
  const cache = new Map();
  const out = [];
  try {
    for (const p of personas) {
      const tl = evaluateLeerlingTimeline(engine, p, PERSONA_PEILDATA, cache);
      const tellend = tl.filter((u) => u.telt);
      const eerste = tellend[0] ?? tl.find((u) => u.in_bestand) ?? null;
      const fout = tl.find((u) => u.fout);
      out.push({
        naam: p.naam,
        sector: p.sector,
        school: p.school_id,
        categorie: eerste ? categorieLabel(eerste.categorie) : null,
        aftrek_kwartalen: eerste?.aftrek_kwartalen ?? null,
        bekostigbare_kwartalen: eerste?.bekostigbare_kwartalen ?? null,
        tellende_peildata: tellend.map((u) => u.peildatum),
        totaal: euro(tellend.reduce((s, u) => s + u.bedrag, 0)),
        ...(fout ? { fout: fout.fout } : {}),
      });
    }
  } finally {
    engine.free?.();
  }
  emit({ type: 'simulatie', doel: 'personas' });
  return JSON.stringify(out);
}

function samenvatting(metrics) {
  const jaren = metrics.jaren;
  const perJaar = {};
  for (const j of jaren) {
    const m = metrics.perJaar[j];
    perJaar[j] = {
      regeling_po: mln(m.regeling_po.totaal),
      regeling_po_asielzoekers: mln(m.regeling_po.per_categorie.ASIELZOEKER),
      regeling_po_overige_vreemdelingen: mln(m.regeling_po.per_categorie.OVERIGE_VREEMDELING),
      regeling_po_tweede_jaar: mln(m.regeling_po.tweede_jaar),
      regeling_vo: mln(m.regeling_vo.totaal),
      uitvoeringslast_school: mln(m.uitvoeringslast.school),
      uitvoeringslast_duo: mln(m.uitvoeringslast.duo),
      investering: mln(m.investering),
      leerlingen_bekostigd_po: Math.round(m.leerlingen_bekostigd.po),
      leerlingen_bekostigd_vo: Math.round(m.leerlingen_bekostigd.vo),
      leerlingen_onder_drempel: Math.round(m.leerlingen_onder_drempel),
      scholen_onder_drempel: Math.round(m.scholen_onder_drempel),
      binnen_budget: m.binnen_budget,
    };
  }
  const n = jaren.length || 1;
  const t = metrics.totaal;
  return {
    per_jaar: perJaar,
    gemiddeld_per_jaar: {
      regeling_po: mln(t.regeling_po.totaal / n),
      regeling_vo: mln(t.regeling_vo.totaal / n),
      uitvoeringslast_school: mln(t.uitvoeringslast.school / n),
      uitvoeringslast_duo: mln(t.uitvoeringslast.duo / n),
      investering: mln(t.investering / n),
    },
    stabiliteit_per_school: metrics.stabiliteit,
    engine_fouten: metrics.fouten,
  };
}

async function simuleerPopulatie({ n, seed }) {
  const distributions = loadDistributions();
  if (!distributions) {
    return 'Geen data/distributions.yaml gevonden; gebruik simuleer_personas.';
  }
  const count = Math.max(50, Math.min(n ?? 300, 2000));
  const records = generatePopulation(distributions, count, seed ?? 42);
  const engine = await createEngine(readOverlays());
  let sim;
  try {
    sim = simulate(engine, records, { jaren: DEFAULT_JAREN });
  } finally {
    engine.free?.();
  }
  const index = Object.fromEntries(
    Object.entries(distributions.bedragen_index_per_jaar ?? {}).map(([j, f]) => [Number(j), Number(f)]),
  );
  // De werkversie van het uitvoeringslastmodel, niet het bronbestand: anders
  // meet de simulatie de handelingen die de assistent zojuist wijzigde niet.
  const werkversie = readHandelingenWerkversie();
  const metrics = aggregate(sim, werkversie ? yamlLoad(werkversie) : loadHandelingen(), { index });
  const jaren = metrics.jaren.length || 1;
  const t = metrics.totaal;
  emit({
    type: 'simulatie',
    doel: 'populatie',
    n: count,
    metrics: {
      totaal: {
        regeling_po: t.regeling_po.totaal / jaren,
        regeling_vo: t.regeling_vo.totaal / jaren,
        uitvoeringslast: t.uitvoeringslast.totaal / jaren,
      },
    },
    // De stand waarop deze meting rust, zodat een punt in het optimalisatiepad
    // te openen en over te nemen is.
    wijzigingen: huidigeWijzigingen(),
    structuurGewijzigd: [...structuurGewijzigd],
    handelingenGewijzigd,
  });
  return JSON.stringify({ n: count, jaren: metrics.jaren, ...samenvatting(metrics) });
}

// ---- Toolregister + JSON-schema's --------------------------------------

const TOOLS = [
  {
    name: 'lees_regelgeving',
    description:
      'Lees de actuele machine-uitvoerbare YAML van een regelgevingsdocument, ' +
      'inclusief eerdere wijzigingen in deze sessie. Zonder document_key: ' +
      'geeft de lijst van beschikbare documenten (Regeling bekostiging WPO en ' +
      'WEC 2025/2026 met art. 34-35 nieuwkomers po, Regeling aanvullende ' +
      'bekostiging eerste opvang nieuwkomers vo, Uitvoeringsbesluit WVO 2020, ' +
      'WPO, WVO 2020, DUO-werkinstructie). Grote documenten worden niet ' +
      'volledig teruggegeven; gebruik "zoek" om een parameter te lokaliseren ' +
      '(bijvoorbeeld bedrag_per_asielzoeker of drempel) en dan offset/limit om ' +
      'het relevante venster genummerd te lezen.',
    inputSchema: {
      type: 'object',
      properties: {
        document_key: {
          type: 'string',
          description: 'Documentsleutel, bv. "regeling_bekostiging_wpo_en_wec@2026-01-01"',
        },
        zoek: {
          type: 'string',
          description:
            'Regex (hoofdletterongevoelig); geeft alle matchende regels met ' +
            'regelnummer terug, zodat je een parameter kunt vinden zonder het ' +
            'hele document te lezen.',
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
    name: 'wijzig_definitie',
    description:
      'Wijzig één waarde in de definitions van een artikel: bedragen (eurocent ' +
      'per leerling per jaar), kwartalen, de drempel, termijnen. Dit is de ' +
      'snelle en veilige weg voor parameterwijzigingen: één aanroep, geen ' +
      'document herschrijven. Vind eerst de naam en het artikel met ' +
      'lees_regelgeving (zoek=...); het artikelnummer is een string zoals "34" ' +
      'of "34, tweede lid". De nieuwe waarde wordt door de engine gevalideerd.',
    inputSchema: {
      type: 'object',
      properties: {
        document_key: { type: 'string', description: 'bv. "regeling_bekostiging_wpo_en_wec@2026-01-01"' },
        artikel: { type: 'string', description: 'artikelnummer zoals in de YAML, bv. "34, tweede lid"' },
        naam: { type: 'string', description: 'naam van de definitie, bv. "drempel_aantal_leerlingen"' },
        waarde: { type: 'number', description: 'nieuwe waarde (eurocent voor bedragen)' },
        toelichting: { type: 'string', description: 'Korte omschrijving van de wijziging, voor de gebruiker' },
      },
      required: ['document_key', 'artikel', 'naam', 'waarde'],
      additionalProperties: false,
    },
    run: wijzigDefinitie,
  },
  {
    name: 'wijzig_regelgeving',
    description:
      'Alleen voor STRUCTUURwijzigingen (een voorwaarde erbij, een lid dat ' +
      'vervalt, een andere formule); voor waarden gebruik je wijzig_definitie. ' +
      'Vervang de VOLLEDIGE YAML van een regelgevingsdocument (niet enkel het ' +
      'gewijzigde fragment). Bij een groot document: lees het eerst helemaal in ' +
      'met offset/limit-vensters, pas het doelfragment aan, en stuur de complete ' +
      'YAML terug. De nieuwe versie wordt eerst door de regelrecht-engine ' +
      'gevalideerd; bij een fout blijft de oude versie gelden en krijg je de ' +
      'foutmelding terug. Wijzig bedragen (eurocent per leerling per jaar), ' +
      'kwartalen en de drempel via definitions en structuur via de operation ' +
      'trees; houd artikelnummers en wettekst-referenties intact. De 2025- en ' +
      '2026-versie zijn aparte documenten; de simulatie 2025-2028 gebruikt de ' +
      '2026-versie voor 2026 en later.',
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
    name: 'lees_handelingen',
    description:
      'Lees het uitvoeringslastmodel (data/handelingen.yaml): welke handelingen ' +
      'de uitvoering kosten, met partij (school, duo, ocw), sector (po, vo, of ' +
      'leeg voor allebei), aanleiding, minuten en tarief, plus de tarieven in ' +
      'eurocent per uur. Dit staat NAAST de regelgeving: wie of wat er in de ' +
      'praktijk aan te pas komt (een accountantscontrole, een aanvraagformulier, ' +
      'een DUO-uitlezing) staat hier en niet in de wet. Een instructie over wie ' +
      'er werk moet verzetten wijzig je dus hier, met wijzig_handelingen.',
    inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    run: leesHandelingen,
  },
  {
    name: 'wijzig_handelingen',
    description:
      'Wijzig het uitvoeringslastmodel: een handeling verwijderen, toevoegen, ' +
      'een veld aanpassen (minuten, tarief, partij, sector, aanleiding, ' +
      'omschrijving) of een tarief wijzigen. Gebruik dit als de instructie over ' +
      'de UITVOERING gaat -- "er hoeft in het po geen accountant aan te pas te ' +
      'komen" verwijdert de po-handeling met tarief accountant, en raakt de ' +
      'regelgeving niet. Lees eerst lees_handelingen voor de id\'s. Reken het ' +
      'effect daarna door met simuleer_populatie: de uitvoeringslast bij ' +
      'scholen en DUO verandert mee. Elke actie heeft zijn eigen verplichte ' +
      'argumenten en accepteert die van een andere actie niet: verwijder heeft ' +
      'id, voeg_toe heeft handeling, wijzig heeft id en velden, wijzig_tarief ' +
      'heeft tarief en waarde.',
    inputSchema: {
      type: 'object',
      properties: {
        actie: {
          type: 'string',
          enum: ACTIES,
          description: 'Wat je wilt doen. Bepaalt welke andere argumenten verplicht zijn.',
        },
        id: { type: 'string', description: 'Verplicht bij actie=verwijder en actie=wijzig: id van de handeling.' },
        velden: {
          type: 'object',
          description:
            'Verplicht bij actie=wijzig: de velden die je wilt zetten, bv. ' +
            '{"minuten": 45} of {"sector": null} om de handeling voor po en vo ' +
            'te laten tellen. Toegestaan zijn omschrijving, partij, sector, ' +
            'aanleiding, minuten, tarief en vanaf_jaar; een ander veld wordt geweigerd.',
          additionalProperties: true,
        },
        handeling: {
          type: 'object',
          description:
            'Verplicht bij actie=voeg_toe: de nieuwe handeling met id, partij, ' +
            'aanleiding, minuten en tarief (sector en vanaf_jaar optioneel).',
          additionalProperties: true,
        },
        tarief: { type: 'string', description: 'Verplicht bij actie=wijzig_tarief: de tariefnaam.' },
        waarde: { type: 'number', description: 'Verplicht bij actie=wijzig_tarief: eurocent per uur.' },
        toelichting: { type: 'string', description: 'Korte omschrijving voor de gebruiker.' },
      },
      required: ['actie'],
      additionalProperties: false,
    },
    run: wijzigHandelingen,
  },
  {
    name: 'simuleer_personas',
    description:
      "Reken de casussen (persona's uit de OCW/DUO-visual, po en vo) door " +
      'over de peildata 1 juli 2024 t/m 1 oktober 2028 onder de actuele ' +
      'regelgeving (inclusief jouw wijzigingen). Geeft per persona categorie, ' +
      'aftrek voor de vierde verjaardag, bekostigbare kwartalen, de tellende ' +
      'peildata en het totaalbedrag.',
    inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    run: simuleerPersonas,
  },
  {
    name: 'simuleer_populatie',
    description:
      'Simuleer een synthetische populatie nieuwkomers (instroom 2023-2028, ' +
      'po en vo, met scholen) over de peildata 2025-2028 onder de actuele ' +
      'regelgeving en geef per jaar de twee rekeningen terug: regeling-uitgaven ' +
      '(po per categorie en tweede jaar, vo), uitvoeringslast bij scholen en ' +
      'DUO, investering, leerlingen bekostigd en onder de drempel, en de ' +
      'budgettoets. Gebruik een bescheiden n (standaard 300) in een ' +
      'optimalisatielus; maximaal 2000 voor een eindmeting.',
    inputSchema: {
      type: 'object',
      properties: {
        n: { type: 'integer', description: 'Aantal synthetische leerlingrecords (50-2000)' },
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
