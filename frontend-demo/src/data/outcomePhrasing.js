/**
 * How a tile tells its outcome.
 *
 * Without this every tile says the same thing: "U voldoet aan de voorwaarden",
 * the amount, and the field name underneath. That is a label, not an answer —
 * the citizen has to work out for themselves what it means for them. The POC
 * wrote a sentence around the number instead ("Uw huurtoeslag is waarschijnlijk
 * € 302,96 per jaar", "Voor de verkiezingen van 29 oktober 2025 heeft u
 * STEMRECHT"), and that reads as something addressed to this person.
 *
 * The wording is content, not code: it lives in `demo-config.yaml` under
 * `outcome_phrasing`, keyed by service and law path. A law that has no entry
 * keeps the general rendering, so this can grow law by law.
 */

/** The phrasing for one law, or null when it has none. */
export function phrasingFor(config, service, lawPath) {
  return config?.outcome_phrasing?.[`${service}/${lawPath}`] ?? null;
}

/**
 * The sentence a tile shows, or null to fall back to the general rendering.
 *
 * @param {object|null} phrasing   the law's entry in `outcome_phrasing`
 * @param {object} outcome
 * @param {boolean} outcome.met        did the law's conditions hold
 * @param {string|null} outcome.value  the formatted amount, already in euros
 * @param {boolean} outcome.isYesNo    is the primary output a yes/no
 * @param {string|null} outcome.date   formatted date for `{date}` in the lead
 * @returns {{lead: string, headline: string, unit: string|null}|null}
 */
export function phraseOutcome(phrasing, { met, value, isYesNo = false, date = null } = {}) {
  if (!phrasing) return null;

  // A yes/no law states the verdict itself; there is no amount to show.
  if (isYesNo || phrasing.yes || phrasing.no) {
    const headline = met ? phrasing.yes : phrasing.no;
    if (!headline || !phrasing.lead) return null;
    return { lead: fillDate(phrasing.lead, date, phrasing.lead_no_date), headline, unit: null };
  }

  // Conditions not met, or nothing granted: one plain sentence, no number.
  if (!met || value === null || value === undefined) {
    return phrasing.none ? { lead: '', headline: phrasing.none, unit: null } : null;
  }

  if (!phrasing.lead) return null;
  return { lead: fillDate(phrasing.lead, date, phrasing.lead_no_date), headline: value, unit: phrasing.unit ?? null };
}

/**
 * Put the date the outcome is about into the lead.
 *
 * Without a date the whole clause around the placeholder goes, not just the
 * placeholder: "Voor de verkiezingen van {date} heeft u" has to become "Voor
 * de verkiezingen heeft u", never "Voor de verkiezingen van heeft u". A
 * `lead_no_date` in the config says what to fall back to, because only the
 * author of the sentence knows which words belonged to the date. Without one
 * the placeholder is dropped and the whitespace collapsed, which is right for
 * a lead that ends on the date ("Per {date} heeft u" → "Per heeft u" is still
 * wrong, so such a lead should carry `lead_no_date`).
 */
function fillDate(lead, date, fallback = null) {
  if (!lead.includes('{date}')) return lead;
  if (!date) return (fallback ?? lead.replace('{date}', '')).replace(/\s{2,}/g, ' ').trim();
  return lead.replace('{date}', date).replace(/\s{2,}/g, ' ').trim();
}

/** Which input carries the date a yes/no law is about, if any. */
export function dateInputFor(phrasing) {
  return phrasing?.date_input ?? null;
}
