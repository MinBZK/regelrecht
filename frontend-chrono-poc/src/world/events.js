/**
 * De waarde uit een event van een ontwerpsysteem-veld.
 *
 * De componenten leveren hun waarde in `event.detail`; de native input eronder
 * heeft er ook een. Defensief lezen is het patroon dat het ontwerpsysteem
 * voorschrijft, en één plek ervoor scheelt vijf varianten in de formulieren.
 */
export function fieldValue(event, fallback = '') {
  return event?.detail?.value ?? event?.target?.value ?? fallback;
}
