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
  // Vóór het besluit: de aanvraag ligt er en wordt behandeld.
  if (stage === 'AANVRAAG') return 'SUBMITTED';
  if (stage === 'BEHANDELING') return 'IN_REVIEW';
  // Vanaf BESLUIT is er een besluit; of het toe- of afwijst zegt `approved`,
  // niet de fase.
  return 'DECIDED';
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
  return reachedStage(caseRecord, 'BEZWAAR') && !caseRecord?.objection;
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
  return {
    motiveringVereist: out.motivering_vereist ?? null,
    bezwaartermijnWeken: out.bezwaartermijn_weken ?? null,
    bezwaartermijnStart: out.bezwaartermijn_startdatum ?? null,
    bezwaartermijnEinde: out.bezwaartermijn_einddatum ?? null,
  };
}
