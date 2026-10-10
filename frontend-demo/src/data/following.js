/**
 * Wat de klok met de besluiten na het eerste doet, los van de store: welke
 * besluiten de klok zelf neemt, wanneer, en wat ze met de uitkomst doet. De
 * store vraagt de cel en legt vast; deze functies kiezen alleen.
 */

import { decisionDue } from './moments.js';

/**
 * Of het volgende besluit `f` (`{day, dates, decidedOn}`, van
 * `followingDecisionsOf`) op `today` genomen wordt: op de dag die het beleid
 * van de houder geeft. Wijst de stroom zo'n beleid aan (`decidedOn`) en geeft
 * het nog geen dag, dan niet: de cel zou het weigeren. Anders als elke datum
 * die het dossier ervoor geeft is geweest.
 */
export function followingDue(f, today) {
  if (f.day) return f.day <= today;
  return !f.decidedOn && decisionDue(f.dates, today);
}

/**
 * Wat de klok doet met een volgend besluit waarvan het moment er is, gezien
 * het oordeel van de wet (`verdictOf` van een voorbeeld):
 *
 * - `'undecided'`: de wet kan nog niet beslissen (er ontbreken gegevens); er
 *   komt geen gram, en de zaak zegt over welk jaar;
 * - `'grant'` of `'refusal'`: het besluit wordt vastgelegd met zijn uitkomst.
 *   Ook een afwijzing is een besluit (Awb 1:3): vastgelegd is de periode
 *   beslist, en volgt de volgende. Zonder gram zou de cel dezelfde periode
 *   blijven geven, en kwam er geen later jaar en geen terugvordering.
 */
export function followingOutcome(verdict) {
  if (verdict === 'unknown') return 'undecided';
  return verdict === false ? 'refusal' : 'grant';
}

/**
 * De parameters van een volgend besluit: wat het dossier geeft, met de
 * periode die de cel gaf in de parameter die de periode geeft (`parameter`,
 * uit de vorm van het besluit). Zonder periode of parameter onveranderd.
 */
export function withPeriod(inputs, parameter, period) {
  if (!parameter || !period) return inputs;
  return { ...inputs, [parameter]: { value: period.value, provenance: { source: 'period' } } };
}

/**
 * Of de gebeurtenis `event` (met haar vorm `shape`) een besluit is dat de
 * klok neemt en de levensloop van de zaak niet: een besluit (decretogram)
 * met een dag van de houder (`decided_on`) dat naar de aanvraag van de wet
 * verwijst (`applicationArticle`), en dat niet de aanvraag of een besluit
 * van de levensloop is (`own`). De terugvordering van Awir 26.
 */
export function isClockDecision(event, shape, applicationArticle, own) {
  if (own.has(event.name) || !event.decided_on || !shape) return false;
  const onApplication = Object.values(shape.refers_to ?? {}).some((r) => r.required && r.to === applicationArticle);
  return shape.type === 'decretogram' && onApplication;
}

/**
 * De ambtshalve besluiten (`exOfficioDue`) die de klok op `today` neemt: met
 * een dag op of vóór vandaag, een periode en de parameter die haar geeft.
 */
export function exOfficioToTake(due, today) {
  return due.filter((d) => d.day && d.day <= today && d.period && d.periodParameter);
}

/**
 * De zaak waarop een ambtshalve besluit over de persoon `bsn` uitkomt: een
 * zaak van die persoon met een aanvraag in een kroniek die niet is
 * ingetrokken. `null` als er geen is.
 */
export function caseOfSubject(cases, bsn, statusOf) {
  return cases.find((c) => c.bsn === bsn && c.applicationGramId && statusOf(c) !== 'WITHDRAWN') ?? null;
}
