/**
 * De levensloop van een besluit, zoals de Awb die geeft (RFC-008).
 *
 * De Awb bepaalt dat een beschikking niet op één moment ontstaat maar door
 * fasen loopt: een aanvraag, de behandeling ervan, het besluit, de
 * bekendmaking, en daarna de bezwaartermijn. De fasen staan niet hier maar in
 * de wet (`procedure:` in de Awb-YAML); de engine loopt ze af en zegt bij elke
 * fase welk gegeven hij nog niet heeft.
 *
 * Dit bestand is de kant van de demo: hoe een zaak in het scherm zich verhoudt
 * tot die levensloop. RFC-008 zegt daarover twee dingen die hier alles bepalen.
 *
 * Ten eerste: "the besluit itself is the state container; there is no separate
 * 'case' or 'zaak' abstraction". De zaak in de demo ís het besluit. Er staat
 * dus geen tweede boekhouding naast de levensloop — de fase is de status, en
 * niet iets wat ernaast wordt bijgehouden en uit de pas kan gaan lopen.
 *
 * Ten tweede: "the engine itself remains stateless ... the orchestration layer
 * persists the besluit state and feeds new inputs when they become available".
 * De demo is die orkestratielaag. Zij bewaart `stageState` bij de zaak en levert
 * aan wat de engine vraagt, op het moment dat het er is.
 *
 * Wat een fase is en wat een uitkomst is, is niet hetzelfde. "Toegekend" en
 * "afgewezen" zijn geen fasen: het zijn twee uitkomsten van dezelfde fase
 * (BESLUIT). Daarom blijft `approved` een veld op het besluit en wordt alleen
 * het moment door de fase beschreven.
 */

/** De fasen van de beschikkingsprocedure, in de volgorde van de Awb. */
export const STAGES = ['AANVRAAG', 'BEHANDELING', 'BESLUIT', 'BEKENDMAKING', 'BEZWAAR'];

/** De fase waarop de demo een afgeronde levensloop zet (`advanceLifecycle`). */
export const COMPLETE = 'BEZWAAR';

/**
 * Welke fase eerder komt dan welke. Alleen voor "is deze zaak al voorbij X",
 * nooit om zelf een volgende fase te kiezen: dat doet de engine, uit de wet.
 */
export function stageIndex(stage) {
  return STAGES.indexOf(stage);
}

export function reachedStage(caseRecord, stage) {
  const at = stageIndex(caseRecord?.stageState?.current_stage);
  return at >= 0 && at >= stageIndex(stage);
}

/**
 * De status waarop de rest van de demo stuurt, afgeleid uit de fase.
 *
 * Afgeleid en niet apart bewaard: twee velden die hetzelfde zouden moeten
 * zeggen, gaan een keer uit de pas lopen. De fase is de bron.
 *
 * Een zaak zonder levensloop (een wet die geen beschikking geeft, of een zaak
 * uit opgeslagen staat van vóór deze verandering) houdt de status die er al
 * stond. Zo blijft een demo die midden in een verhaal staat gewoon werken.
 */
export function statusOf(caseRecord) {
  if (caseRecord?.withdrawnAt) return 'WITHDRAWN';
  const stage = caseRecord?.stageState?.current_stage;
  if (!stage) return caseRecord?.status ?? 'IN_REVIEW';
  // `current_stage` is de fase waar de levensloop vóór staat, niet de fase die
  // hij heeft afgerond: de engine zet hem op de volgende fase en kijkt dán pas
  // of hij de gegevens daarvan heeft (`execute_stage_internal`). Een zaak die
  // op BESLUIT staat wacht dus nog op de besluitdatum — de haken van artikel
  // 3:46 en 6:7 hebben niet gevuurd, en er ís nog geen besluit.
  //
  // Dat verschil verkeerd lezen draait de demo om: elke aanvraag die naar een
  // behandelaar gaat, zou meteen als besloten op het portaal staan.
  //
  // Wat een fase is, zegt de procedure: een fase VOORSCHOT van de Awir `is`
  // een BESLUIT. Zonder procedure bij de zaak gelden de fasen van de Awb.
  const stages = caseRecord.procedureStages ?? STAGES.map((name) => ({ name }));
  const at = stages.findIndex((s) => s.name === stage);
  // Een fase buiten de procedure: de levensloop is klaar.
  if (at < 0) return 'DECIDED';
  // Voorbij een besluit is het besluit genomen. Of het toe- of afwijst zegt
  // `approved`, niet de fase.
  if (stages.slice(0, at).some((s) => stageKind(s) === 'BESLUIT')) return 'DECIDED';
  if (stageKind(stages[at]) === 'AANVRAAG') return 'SUBMITTED';
  return 'IN_REVIEW';
}

/** Wat een fase is: haar `is` (VOORSCHOT is een BESLUIT), anders haar naam. */
function stageKind(stage) {
  return stage?.is ?? stage?.name;
}

/**
 * De fasen van procedure `procedureId`, uit de wet die haar vastlegt
 * (`procedure:` in de YAML): per fase de naam, wat ze `is` en welke gegevens
 * ze vraagt (`requires`). `null` als geen wet in `lawDocs` die procedure kent.
 */
export function procedureStages(lawDocs, procedureId) {
  if (!procedureId) return null;
  for (const doc of lawDocs ?? []) {
    const procedure = (doc?.procedure ?? []).find((p) => p.id === procedureId);
    if (procedure) {
      return procedure.stages.map((s) => ({
        name: s.name,
        is: s.is ?? null,
        requires: (s.requires ?? []).map((r) => ({ name: r.name, type: r.type ?? null })),
      }));
    }
  }
  return null;
}

/**
 * Wat de demo eerder aan de levensloop leverde en nu weer mag meegeven: alles,
 * behalve wat een fase vraagt die nog moet komen.
 *
 * Een gegeven dat een fase vraagt, bestaat op het moment van die fase. Twee
 * fasen kunnen hetzelfde vragen: in de procedure van de Awir wordt het
 * voorschot bekendgemaakt en later de toekenning, en beide vragen een
 * bekendmakingsdatum (Awb 6:8). De datum van de eerste bekendmaking zou de
 * tweede anders meteen laten gebeuren.
 */
export function carriedInputs(inputs, stages, currentStage) {
  const at = (stages ?? []).findIndex((s) => s.name === currentStage);
  if (at < 0) return { ...(inputs ?? {}) };
  const coming = new Set(stages.slice(at).flatMap((s) => (s.requires ?? []).map((r) => r.name)));
  return Object.fromEntries(Object.entries(inputs ?? {}).filter(([name]) => !coming.has(name)));
}

/**
 * Of het laatste besluit van de zaak is bekendgemaakt: de laatste fase die de
 * zaak voorbij is en een besluit of een bekendmaking `is`, is een
 * bekendmaking. Per besluit, dus na een tweede besluit (de toekenning na het
 * voorschot) weer niet, tot ook dat is bekendgemaakt.
 */
export function announced(caseRecord) {
  const stage = caseRecord?.stageState?.current_stage;
  if (!stage) return false;
  const stages = caseRecord.procedureStages ?? STAGES.map((name) => ({ name }));
  const at = stages.findIndex((s) => s.name === stage);
  // Buiten de procedure staat alleen een afgeronde levensloop (de demo zet
  // hem dan op BEZWAAR): alles is voorbij. Een andere onbekende fase zegt
  // niets, en dan is er niets bekendgemaakt.
  if (at < 0 && stage !== COMPLETE) return false;
  const passed = at < 0 ? stages : stages.slice(0, at);
  const last = [...passed].reverse().find((s) => ['BESLUIT', 'BEKENDMAKING'].includes(stageKind(s)));
  return stageKind(last) === 'BEKENDMAKING';
}

/**
 * Wat een besluit op `date` aanlevert aan de fase waarop de zaak wacht: de
 * dagtekening, en niets anders. Welke parameter dat is, zegt de wet:
 *
 * - legt een cel het besluit vast, dan de parameter die de wet bij het
 *   besluit als zijn dagtekening noemt (`dated_by` in de vorm van de
 *   gebeurtenis, `datedBy`);
 * - anders (de Awb-levensloop van een andere wet) wat de fase vraagt en
 *   volgens de procedure een datum is (`type: date`).
 *
 * Wat de fase verder vraagt, vult de demo niet met een datum in.
 */
export function decisionDates(caseRecord, date, datedBy = null) {
  const pending = caseRecord?.pendingInputs ?? [];
  if (datedBy) return pending.includes(datedBy) ? { [datedBy]: date } : {};
  const stage = (caseRecord?.procedureStages ?? []).find((s) => s.name === caseRecord?.stageState?.current_stage);
  const dates = new Set((stage?.requires ?? []).filter((r) => r.type === 'date').map((r) => r.name));
  return Object.fromEntries(pending.filter((name) => dates.has(name)).map((name) => [name, date]));
}

/**
 * Of de burger nu bezwaar kan maken.
 *
 * Niet "er is besloten", maar "de bezwaartermijn loopt". Awb 6:8 laat die
 * termijn beginnen op de dag ná de bekendmaking, dus vóór de bekendmaking is er
 * niets om bezwaar tegen te maken — het besluit is de belanghebbende dan nog
 * niet eens meegedeeld.
 */
export function objectionOpen(caseRecord) {
  return announced(caseRecord) && !caseRecord?.objection;
}

/**
 * De uitkomsten die de Awb aan dit besluit heeft toegevoegd, als de levensloop
 * ver genoeg is om ze te hebben.
 *
 * Ze komen uit de wet en niet uit dit bestand: `bezwaartermijn_weken` van Awb
 * 6:7, de begin- en einddatum van Awb 6:8, de motiveringsplicht van Awb 3:46.
 * Staat er een afwijkende termijn in een bijzondere wet ("in afwijking van
 * artikel 6:7"), dan is dat hier al in verwerkt zonder dat iemand het hoeft te
 * weten.
 */
export function awbOutcomes(caseRecord) {
  const out = caseRecord?.stageState?.accumulated_outputs ?? {};
  // De termijn hoort bij het besluit dat is bekendgemaakt. Is er daarna een
  // nieuw besluit genomen, dan is de termijn van het vorige niet die van dit.
  const current = announced(caseRecord);
  return {
    motiveringVereist: out.motivering_vereist ?? null,
    bezwaartermijnWeken: out.bezwaartermijn_weken ?? null,
    bezwaartermijnStart: current ? out.bezwaartermijn_startdatum ?? null : null,
    bezwaartermijnEinde: current ? out.bezwaartermijn_einddatum ?? null : null,
  };
}
