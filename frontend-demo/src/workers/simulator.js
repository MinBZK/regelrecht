// Dedicated Web Worker that runs the engine against a synthetic population.
//
// Protocol:
//   Main -> Worker: { type: 'run', lawYaml, lawEntry, population, calculationDate }
//   Worker -> Main (progress): { type: 'progress', done, total }
//   Worker -> Main (result):   { type: 'result', results: [{ amount, eligible }], summary }
//   Worker -> Main (error):    { type: 'error', message }

// Use a dynamic import so the WASM bundle loads lazily in the worker.
let engine = null;

async function ensureEngine() {
  if (engine) return engine;
  const mod = await import('/wasm/pkg/regelrecht_engine.js');
  await mod.default();
  engine = new mod.WasmEngine();
  return engine;
}

function personRecords(person) {
  return [
    { service: 'RVIG', table: 'personal_data', key: 'bsn', records: [{
      bsn: person.bsn,
      geboortedatum: person.geboortedatum,
      verblijfsadres: 'Amsterdam',
      land_verblijf: 'NEDERLAND',
    }] },
    { service: 'RVIG', table: 'relationship_data', key: 'bsn', records: [{
      bsn: person.bsn,
      partnerschap_type: person.hasPartner ? 'GEREGISTREERD_PARTNERSCHAP' : 'GEEN',
      partner_bsn: person.partner_bsn ?? null,
    }] },
    { service: 'RVZ', table: 'insurance', key: 'bsn', records: [{
      bsn: person.bsn,
      polis_status: 'ACTIEF',
      verdragsinschrijving: false,
    }] },
    { service: 'BELASTINGDIENST', table: 'box1', key: 'bsn', records: [{
      bsn: person.bsn,
      loon_uit_dienstbetrekking: person.inkomen,
      uitkeringen_en_pensioenen: 0,
      winst_uit_onderneming: 0,
      resultaat_overige_werkzaamheden: 0,
      eigen_woning: 0,
      buitenlands_inkomen: 0,
    }] },
    { service: 'BELASTINGDIENST', table: 'box2', key: 'bsn', records: [{
      bsn: person.bsn,
      reguliere_voordelen: 0,
      vervreemdingsvoordelen: 0,
    }] },
    { service: 'BELASTINGDIENST', table: 'box3', key: 'bsn', records: [{
      bsn: person.bsn,
      spaargeld: person.vermogen,
      beleggingen: 0,
      onroerend_goed: 0,
      schulden: 0,
    }] },
    { service: 'DJI', table: 'detenties', key: 'bsn', records: [{
      bsn: person.bsn,
      detentiestatus: null,
      inrichting_type: null,
      zorgtype: null,
      juridische_grondslag: null,
    }] },
  ];
}

async function runSimulation({ lawYaml, lawEntry, population, calculationDate }) {
  const e = await ensureEngine();
  if (!e.hasLaw(lawEntry.id)) e.loadLaw(lawYaml);

  const results = new Array(population.length);
  let eligible = 0;
  let totalAmount = 0;

  for (let i = 0; i < population.length; i++) {
    const person = population[i];
    e.clearDataSources();
    for (const ds of personRecords(person)) {
      e.registerDataSource(ds.table, ds.key, ds.records);
    }
    let amount = 0;
    let err = null;
    try {
      const result = e.execute(
        lawEntry.id,
        lawEntry.output,
        { bsn: person.bsn },
        calculationDate,
      );
      const raw = result?.outputs?.[lawEntry.output];
      amount = typeof raw === 'number' ? raw : Number(raw ?? 0);
    } catch (ex) {
      err = ex?.message || String(ex);
    }
    const isEligible = !err && amount > 0;
    results[i] = { bsn: person.bsn, amount, eligible: isEligible, error: err };
    if (isEligible) {
      eligible += 1;
      totalAmount += amount;
    }
    if (i % 50 === 49 || i === population.length - 1) {
      postMessage({ type: 'progress', done: i + 1, total: population.length });
    }
  }

  const amounts = results.filter((r) => r.eligible).map((r) => r.amount).sort((a, b) => a - b);
  const median = amounts.length ? amounts[Math.floor(amounts.length / 2)] : 0;

  postMessage({
    type: 'result',
    results,
    summary: {
      total: population.length,
      eligible,
      percentageEligible: population.length ? eligible / population.length : 0,
      averageAmount: eligible ? totalAmount / eligible : 0,
      medianAmount: median,
    },
  });
}

onmessage = (event) => {
  const msg = event.data;
  if (msg?.type === 'run') {
    runSimulation(msg).catch((err) => {
      postMessage({ type: 'error', message: err?.message || String(err) });
    });
  }
};
