/**
 * Het transport tussen de cellen van de demo: wat de ene cel vastlegt en de
 * andere ontvangt (een kanaal, RFC-022). Toeslagen geeft een betaalopdracht,
 * de bank ontvangt haar en schrijft bij of weigert, en Toeslagen ontvangt dat
 * antwoord.
 *
 * Dit is orkestratie, geen wet. Welke gram naar welke cel gaat en welk veld
 * welke parameter van het ontvangende artikel vult, staat in `channels` in
 * demo-config.yaml. Wat er bij de ontvanger ontstaat, zegt zijn wet: de cel
 * voert het artikel uit en legt vast wat het zegt (`WasmCell.receive`). Dit
 * bestand kent geen gebeurtenis en geen veld bij naam.
 *
 * Een kanaal:
 *
 * ```yaml
 * - from: {cell: toeslagen, event: betaalopdracht_gegeven}   # of `article`
 *   to: {cell: bank, article: fictieve_bankvoorwaarden#1, refers_to: {naam: veld}}
 *   inputs: {parameter: veld}   # `$id` is het kenmerk van de gram
 * ```
 */

/** Hoe diep een antwoord op een antwoord mag gaan voordat het een lus is. */
const MAX_HOPS = 8;

/** Of `gram` van cel `cellId` over kanaal `channel` vertrekt. */
export function leaves(channel, cellId, gram) {
  const from = channel?.from ?? {};
  if (from.cell !== cellId) return false;
  if (from.event && from.event !== gram?.name) return false;
  if (from.article && from.article !== gram?.establishes) return false;
  return !!(from.event || from.article);
}

/** De waarde van `source` (een veldnaam of `$id`) in `gram`; undefined als die er niet is. */
function valueOf(gram, source) {
  if (source === '$id') return gram.id;
  return gram.fields?.[source];
}

/**
 * Het bericht dat kanaal `channel` maakt van `gram` van cel `cellId`: per
 * parameter van het ontvangende artikel de waarde met waar zij vandaan komt,
 * en de verwijzingen naar grammen van de ontvanger. Een veld dat de gram niet
 * heeft, gaat niet mee (de wet van de ontvanger zegt dan wat dat betekent).
 */
export function messageFor(channel, cellId, gram) {
  const inputs = {};
  for (const [parameter, source] of Object.entries(channel.inputs ?? {})) {
    const value = valueOf(gram, source);
    if (value === undefined) continue;
    inputs[parameter] = { value, provenance: { source: 'kanaal', from: cellId, gram: gram.id, field: source } };
  }
  const refersTo = {};
  for (const [name, source] of Object.entries(channel.to?.refers_to ?? {})) {
    const value = valueOf(gram, source);
    if (typeof value === 'string' && value) refersTo[name] = value;
  }
  return { inputs, refersTo };
}

/**
 * Bezorg wat `gram` van cel `cellId` over de kanalen naar andere cellen
 * brengt, en wat daar ontstaat weer verder, tot er niets meer volgt.
 * `receive(cellId, article, refersTo, inputs)` laat de ontvangende cel het
 * artikel uitvoeren en geeft de grammen die zij vastlegde. Geeft elke
 * vastgelegde gram terug, met de cel die hem vastlegde. Een fout van een cel
 * breekt het transport af: de fout gaat naar de aanroeper.
 */
export function deliver(gram, cellId, channels, receive, hops = 0) {
  if (hops >= MAX_HOPS) throw new Error(`Kanalen lopen rond na ${MAX_HOPS} stappen (${cellId}: ${gram?.name})`);
  const recorded = [];
  for (const channel of channels ?? []) {
    if (!leaves(channel, cellId, gram)) continue;
    const { inputs, refersTo } = messageFor(channel, cellId, gram);
    const to = channel.to.cell;
    for (const g of receive(to, channel.to.article, refersTo, inputs) ?? []) {
      recorded.push({ cellId: to, gram: g });
      recorded.push(...deliver(g, to, channels, receive, hops + 1));
    }
  }
  return recorded;
}
