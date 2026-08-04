/**
 * corpusgraaf.js — de afhankelijkheidsgraaf van een heel corpus, met de
 * integriteit van elke binding (bouwplan §3.1: "Cross-law-integriteit …
 * bestaand algoritme, ongewijzigd overnemen" en "Afhankelijkheidsgraaf").
 *
 * Getrouwe port van `script/cross-law-integriteit.py`. Die klassen zijn niet
 * bedacht maar geobserveerd, en elk ervan beschrijft een manier waarop een
 * binding er wél staat maar níet werkt:
 *
 * - `misplaced`     — `source:` onder `parameters:` in plaats van onder
 *                     `input:`. De engine kent geen `source` op een Parameter,
 *                     dus de binding wordt bij het parsen stil weggegooid en de
 *                     waarde wordt een gewone directe parameter.
 * - `dangling`      — een `source: {regulation, output}` waarvan de doelwet die
 *                     output niet produceert; de engine faalt bij resolutie.
 * - `plain-param`   — een parameter die in zijn beschrijving een andere
 *                     regeling noemt maar geen `source:` heeft.
 * - `impl-dangling` — een `implements` die wijst naar een wet/artikel dat die
 *                     `open_term` niet declareert; de delegatie resolvet nooit.
 * - `impl-no-date`  — een regeling met `implements` maar zonder `valid_from`.
 *                     RFC-003's temporele filter matcht 'm dan op élke
 *                     rekendatum en overschrijft stil de juiste versie.
 *
 * ## Waarom een misplaced binding géén rand is
 *
 * De verleiding is om hem als rode pijl te tekenen. Dat zou precies de leugen
 * zijn waar §2 van het bouwplan voor waarschuwt: de engine ziet die rand niet,
 * dus een graaf die hem tekent toont een samenhang die bij uitvoering niet
 * bestaat. Een misplaced binding is daarom een **bevinding op de knoop**, met
 * de wet waar de rand héén zou hebben gewezen erbij vermeld.
 *
 * Determinisme (§5): elke lijst wordt totaal gesorteerd voor teruggave.
 */

/** Beschrijvingen die zeggen "dit hoort een cross-law binding te zijn". */
const PLAIN_MARKERS = ['conceptueel', 'tijdelijk als directe parameter'];

/** Alle outputs die een wet produceert, uit `actions[].output` en `output[].name`. */
export function outputsVan(doc) {
  const out = new Set();
  for (const art of doc?.articles ?? []) {
    const ex = art?.machine_readable?.execution ?? {};
    for (const a of ex.actions ?? []) if (a && typeof a === 'object' && 'output' in a) out.add(a.output);
    for (const o of ex.output ?? []) if (o && typeof o === 'object' && o.name) out.add(o.name);
  }
  return out;
}

/** Per artikelnummer de gedeclareerde `open_term`-ids. */
export function openTermsVan(doc) {
  const idx = new Map();
  for (const art of doc?.articles ?? []) {
    const num = String(art?.number ?? '?');
    const ids = (art?.machine_readable?.open_terms ?? [])
      .filter((o) => o && typeof o === 'object')
      .map((o) => o.id);
    if (ids.length) idx.set(num, new Set([...(idx.get(num) ?? []), ...ids]));
  }
  return idx;
}

/**
 * Bouw de graaf + integriteitsrapport.
 *
 * @param {Array<{lawId: string, doc: object}>} wetten geparste corpus-YAML's
 * @returns {object} `{ knopen, randen, telling, bevindingen }`
 */
export function bouwGraaf(wetten) {
  const doc = new Map(wetten.map((w) => [w.lawId, w.doc]));
  const outputs = new Map(wetten.map((w) => [w.lawId, outputsVan(w.doc)]));
  const openTerms = new Map(wetten.map((w) => [w.lawId, openTermsVan(w.doc)]));

  const randen = [];
  const bevindingen = [];
  let clean = 0;

  const ids = [...doc.keys()].sort((a, b) => a.localeCompare(b, 'nl'));

  for (const lid of ids) {
    const d = doc.get(lid);
    const artikelen = d?.articles ?? [];

    // IMPL-NO-DATE geldt per wet, niet per artikel.
    const heeftImplements = artikelen.some((a) => a?.machine_readable?.implements?.length);
    if (heeftImplements && !d?.valid_from) {
      bevindingen.push({
        klasse: 'impl-no-date',
        lawId: lid,
        artikel: null,
        tekst: 'implements zonder valid_from (matcht elke rekendatum)',
      });
    }

    for (const art of artikelen) {
      const num = String(art?.number ?? '?');
      const mr = art?.machine_readable ?? {};
      const ex = mr.execution ?? {};

      // IMPL-DANGLING — de IoC-binding moet op een gedeclareerde open_term wijzen.
      for (const im of mr.implements ?? []) {
        if (!im || typeof im !== 'object') continue;
        const { law: tlaw, open_term: term } = im;
        const tart = String(im.article);
        if (!openTerms.has(tlaw)) {
          bevindingen.push({ klasse: 'impl-dangling', lawId: lid, artikel: num, tekst: `implements onbekende wet ${tlaw}` });
          randen.push({ van: lid, naar: tlaw, soort: 'implements', integriteit: 'impl-dangling', label: term ?? '' });
        } else if (!(openTerms.get(tlaw).get(tart) ?? new Set()).has(term)) {
          bevindingen.push({
            klasse: 'impl-dangling',
            lawId: lid,
            artikel: num,
            tekst: `${tlaw} art ${tart} declareert open_term "${term}" niet`,
          });
          randen.push({ van: lid, naar: tlaw, soort: 'implements', integriteit: 'impl-dangling', label: term ?? '' });
        } else {
          clean += 1;
          randen.push({ van: lid, naar: tlaw, soort: 'implements', integriteit: 'clean', label: term ?? '' });
        }
      }

      // MISPLACED / PLAIN-PARAM — beide onder `parameters:`, en beide leveren
      // GEEN rand op: de engine ziet hier niets.
      for (const p of ex.parameters ?? []) {
        if (!p || typeof p !== 'object') continue;
        if (p.source) {
          bevindingen.push({
            klasse: 'misplaced',
            lawId: lid,
            artikel: num,
            naar: p.source.regulation ?? null,
            tekst: `${p.name}: source onder parameters — genegeerd door de engine; verplaats naar input:`,
          });
        } else {
          const beschrijving = String(p.description ?? '').toLowerCase();
          if (PLAIN_MARKERS.some((mk) => beschrijving.includes(mk))) {
            bevindingen.push({ klasse: 'plain-param', lawId: lid, artikel: num, tekst: String(p.name) });
          }
        }
      }

      // DANGLING — echte bindingen onder `input:`, op resolveerbaarheid.
      for (const inp of ex.input ?? []) {
        if (!inp || typeof inp !== 'object') continue;
        const src = inp.source;
        if (!src || typeof src !== 'object') continue;
        const reg = src.regulation ?? null;
        const out = src.output ?? null;
        // `source: {}` is een data-registry-binding, geen cross-law verwijzing.
        if (reg === null && out === null) continue;

        if (reg === null) {
          // Intra-law: verwijst naar een output van de wet zelf.
          if (!(outputs.get(lid) ?? new Set()).has(out)) {
            bevindingen.push({ klasse: 'dangling', lawId: lid, artikel: num, tekst: `intra-law ${out} bestaat niet` });
            randen.push({ van: lid, naar: lid, soort: 'source', integriteit: 'dangling', label: String(out ?? '') });
          } else {
            clean += 1;
            randen.push({ van: lid, naar: lid, soort: 'source', integriteit: 'clean', label: String(out ?? '') });
          }
        } else {
          const doelOk = outputs.has(reg) && (out === null || outputs.get(reg).has(out));
          if (!doelOk) {
            bevindingen.push({ klasse: 'dangling', lawId: lid, artikel: num, tekst: `${reg}.${out} bestaat niet in doelwet` });
            randen.push({ van: lid, naar: reg, soort: 'source', integriteit: 'dangling', label: String(out ?? '') });
          } else {
            clean += 1;
            randen.push({ van: lid, naar: reg, soort: 'source', integriteit: 'clean', label: String(out ?? '') });
          }
        }
      }
    }
  }

  // Knopen: elke wet in het corpus, plus doelwetten die er niet in staan (die
  // zijn zelf de bevinding — een rand naar het niets moet zichtbaar zijn).
  const knoopIds = new Set(ids);
  for (const r of randen) knoopIds.add(r.naar);

  const inkomend = new Map([...knoopIds].map((id) => [id, 0]));
  for (const r of randen) if (r.van !== r.naar) inkomend.set(r.naar, (inkomend.get(r.naar) ?? 0) + 1);

  const knopen = [...knoopIds]
    .sort((a, b) => a.localeCompare(b, 'nl'))
    .map((id) => {
      const d = doc.get(id);
      return {
        lawId: id,
        aanwezig: doc.has(id),
        laag: d?.regulatory_layer ?? null,
        validFrom: d?.valid_from ?? null,
        artikelen: (d?.articles ?? []).length,
        outputs: (outputs.get(id) ?? new Set()).size,
        inkomend: inkomend.get(id) ?? 0,
        // §3.1: "Niet-aangeroepen regelingen | in-degree 0 | gemodelleerd maar
        // buiten elk rekenpad". Een wet zonder inkomende rand is geen fout,
        // maar wel een signaal.
        nietAangeroepen: (inkomend.get(id) ?? 0) === 0,
      };
    });

  const telling = {
    clean,
    misplaced: bevindingen.filter((b) => b.klasse === 'misplaced').length,
    dangling: bevindingen.filter((b) => b.klasse === 'dangling').length,
    'plain-param': bevindingen.filter((b) => b.klasse === 'plain-param').length,
    'impl-dangling': bevindingen.filter((b) => b.klasse === 'impl-dangling').length,
    'impl-no-date': bevindingen.filter((b) => b.klasse === 'impl-no-date').length,
  };

  return {
    knopen,
    randen: randen.sort(
      (a, b) => a.van.localeCompare(b.van, 'nl') || a.naar.localeCompare(b.naar, 'nl') || a.label.localeCompare(b.label, 'nl'),
    ),
    telling,
    bevindingen: bevindingen.sort(
      (a, b) =>
        a.klasse.localeCompare(b.klasse, 'nl') ||
        a.lawId.localeCompare(b.lawId, 'nl') ||
        String(a.artikel ?? '').localeCompare(String(b.artikel ?? ''), 'nl', { numeric: true }),
    ),
  };
}
