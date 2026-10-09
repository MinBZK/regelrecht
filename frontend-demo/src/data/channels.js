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
 *
 * Een bron is een veld van de gram, of `$id` (haar kenmerk), `$period` (de
 * waarde van de periode die zij betreft) of `$input.<naam>` (de waarde
 * waarmee de cel haar parameter `<naam>` vulde).
 *
 * Gaan er van één gram berichten naar meer artikelen (het antwoord van de
 * bank op een termijn of op een nabetaling), dan zegt de ontvangende cel van
 * elk bericht of het voor dat artikel is (`not_addressed`). Een bericht dat
 * voor geen enkel artikel is, komt in de outbox.
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

/**
 * De waarde van `source` in `gram`: een veldnaam, `$id`, `$period` of
 * `$input.<naam>`; undefined als die er niet is.
 */
export function valueOf(gram, source) {
  if (source === '$id') return gram.id;
  if (source === '$period') return gram.period?.value;
  if (typeof source === 'string' && source.startsWith('$input.')) return gram.inputs?.[source.slice('$input.'.length)]?.value;
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
 * De naam van een fout van een cel die zegt dat het bericht al beantwoord is
 * (`Error::Answered` in de cel): het kwam eerder al aan. Voor de afzender is
 * het daarmee bezorgd.
 */
export const ANSWERED = 'answered';

/** De naam van de fout van kanalen die rondlopen: een fout in de configuratie. */
export const LOOP = 'loop';

/**
 * De naam van een fout van een cel die zegt dat het bericht niet voor dit
 * artikel is (`Error::NotAddressed` in de cel): het gaat over een gram die
 * dit artikel niet beantwoordt. Een ander kanaal van dezelfde gram brengt
 * het waar het hoort; alleen als geen enkel kanaal het kwijt kan, is het niet
 * aangekomen.
 */
export const NOT_ADDRESSED = 'not_addressed';

/** Een kanaal bij naam: van welke cel en welk feit, naar welke cel en welk artikel. */
export function channelKey(channel) {
  const from = channel?.from ?? {};
  return `${from.cell}/${from.event ?? from.article} -> ${channel?.to?.cell}/${channel?.to?.article}`;
}

/**
 * Bezorg wat `gram` van cel `cellId` over de kanalen naar andere cellen
 * brengt, en wat daar ontstaat weer verder, tot er niets meer volgt.
 * `receive(cellId, article, refersTo, inputs, sentAt)` laat de ontvangende
 * cel het artikel uitvoeren en geeft de grammen die zij vastlegde; `sentAt`
 * is het moment waarop het bericht vertrok, het moment van de gram die het
 * draagt.
 *
 * Geeft `{ recorded, undelivered }`: elke vastgelegde gram met de cel die
 * hem vastlegde, en elk bericht dat niet aankwam, als
 * `{ cellId, gramId, channel, error, tag }` (de afzender, zijn gram, het
 * kanaal bij naam, de fout, en `tag` van de aanroeper). Een bericht dat niet
 * aankomt, houdt de rest niet op; de aanroeper bewaart het en biedt het later
 * opnieuw aan (`redeliver`). Een bericht dat al beantwoord was, telt als
 * bezorgd. Kanalen die rondlopen zijn een fout in de configuratie: het
 * bericht dat de lus begon, staat dan met die fout bij wat niet aankwam, met
 * `loop` (de kanalen waarin het rondliep).
 */
export function deliver(gram, cellId, channels, receive, tag = {}) {
  const out = { recorded: [], undelivered: [] };
  const missed = [];
  let addressed = 0;
  for (const channel of channels ?? []) {
    if (!leaves(channel, cellId, gram)) continue;
    const entry = { cellId, gramId: gram.id, channel: channelKey(channel), tag };
    attempt(out, entry, channels, () => {
      if (over(channel, gram, cellId, channels, receive, 0, out, tag, gram.effective_at ?? null)) addressed += 1;
      else missed.push(entry);
    });
  }
  unaddressed(out, missed, addressed);
  return out;
}

/**
 * Berichten van één gram die geen ontvanger voor zich hielden: is er geen
 * enkel kanaal waarop het bericht wél aankwam, dan staat het bij wat niet
 * aankwam (met de fout van het eerste kanaal); anders is het bezorgd.
 */
function unaddressed(out, missed, addressed) {
  if (addressed > 0 || !missed.length) return;
  const [first] = missed;
  out.undelivered.push({ ...first, error: `Geen ontvanger voor dit bericht: ${missed.map((m) => m.channel).join(', ')}` });
}

/**
 * Bied wat in `outbox` niet aankwam (de `undelivered` van `deliver`)
 * opnieuw aan, nu: `receive` krijgt voor die eerste stap geen `sentAt`, want
 * het bericht komt pas aan op het moment van bezorgen. Wat er dan ontstaat,
 * gaat weer verder zoals bij `deliver`. `gramOf(cellId, gramId)` geeft de
 * gram van de afzender. Geeft `{ recorded, undelivered }`, met wat nog
 * steeds niet aankwam.
 *
 * Een bericht dat in een lus liep (`loop`), blijft staan zoals het is zolang
 * de kanalen dezelfde zijn: de fout zit in de kanalen, niet in de bezorging,
 * en opnieuw aanbieden zou hem verbergen (de eerste ontvanger antwoordt dat
 * het al beantwoord is). Zijn de kanalen veranderd, dan wordt het opnieuw
 * aangeboden als elk ander bericht, of vervalt het als zijn kanaal er niet
 * meer is.
 */
export function redeliver(outbox, channels, gramOf, receive) {
  const out = { recorded: [], undelivered: [] };
  for (const entry of outbox ?? []) {
    if (entry.loop && entry.loop === fingerprint(channels)) {
      out.undelivered.push(entry);
      continue;
    }
    // Een bericht dat rondliep, kwam zelf wel aan; is zijn kanaal er niet
    // meer, dan is er niets meer te bezorgen.
    if (entry.loop && !(channels ?? []).some((c) => channelKey(c) === entry.channel)) continue;
    attempt(out, entry, channels, () => {
      const channel = (channels ?? []).find((c) => channelKey(c) === entry.channel);
      const gram = gramOf(entry.cellId, entry.gramId);
      if (!channel || !gram) {
        const missing = channel ? `gram ${entry.gramId} van cel ${entry.cellId}` : `kanaal ${entry.channel}`;
        throw new Error(`Niet opnieuw te bezorgen: ${missing} bestaat niet`);
      }
      // In de outbox staat een bericht dat geen enkel kanaal kwijt kon (een
      // fout in de kanalen); zegt de ontvanger nog steeds dat het niet voor
      // haar is, dan blijft het staan.
      if (!over(channel, gram, entry.cellId, channels, receive, 0, out, entry.tag ?? {}, null)) {
        throw new Error(`Geen ontvanger voor dit bericht: ${entry.channel}`);
      }
    });
  }
  return out;
}

/**
 * Eén bericht van de aanroeper (`entry`) proberen: wat daarbij misgaat,
 * houdt de rest niet op. Het bericht blijft dan staan met de fout. Dat het
 * opnieuw aanbieden niets dubbel vastlegt, komt van de ontvanger: elke cel
 * herkent een bericht dat zij al beantwoordde, aan een verplichte
 * verwijzing of aan `identified_by` (de cel weigert een ontvangst zonder
 * een van beide), en antwoordt dan `answered`. Kanalen die rondlopen
 * (`LOOP`) krijgen in `loop` de vingerafdruk van de kanalen waarin ze
 * rondliepen: dat bericht kwam zelf wel aan, dus opnieuw aanbieden lost pas
 * iets op als de kanalen veranderd zijn (zie `redeliver`).
 */
function attempt(out, entry, channels, fn) {
  try {
    fn();
  } catch (e) {
    const { loop: _, ...rest } = entry;
    const failed = { ...rest, error: String(e?.message ?? e) };
    if (e?.name === LOOP) failed.loop = fingerprint(channels);
    out.undelivered.push(failed);
  }
}

/** De kanalen als één tekst, om te zien of ze veranderd zijn. */
function fingerprint(channels) {
  return JSON.stringify(channels ?? []);
}

/**
 * Per zaak (`tag.caseId`) de fout van wat er voor haar in `outbox` staat,
 * als één tekst; een zaak zonder bericht in de outbox staat er niet in. Zo
 * volgt de fout op een zaak de outbox: hij verdwijnt pas als het bericht
 * aankwam.
 */
export function deliveryErrors(outbox) {
  const out = {};
  for (const u of outbox ?? []) {
    const caseId = u.tag?.caseId;
    if (caseId == null) continue;
    out[caseId] = out[caseId] ? `${out[caseId]}; ${u.error}` : u.error;
  }
  return out;
}

function send(gram, cellId, channels, receive, hops, out, tag) {
  if (hops >= MAX_HOPS) {
    const e = new Error(`Kanalen lopen rond na ${MAX_HOPS} stappen (${cellId}: ${gram?.name})`);
    e.name = LOOP;
    throw e;
  }
  const missed = [];
  let addressed = 0;
  for (const channel of channels ?? []) {
    if (!leaves(channel, cellId, gram)) continue;
    if (over(channel, gram, cellId, channels, receive, hops, out, tag, gram.effective_at ?? null)) addressed += 1;
    else missed.push({ cellId, gramId: gram.id, channel: channelKey(channel), tag });
  }
  unaddressed(out, missed, addressed);
}

/**
 * Eén bericht over één kanaal, en wat daar ontstaat weer verder. Geeft
 * `false` als de ontvanger zegt dat het bericht niet voor dit artikel is
 * (`not_addressed`), anders `true` (ook als het niet aankwam: dat staat dan
 * bij wat niet aankwam).
 */
function over(channel, gram, cellId, channels, receive, hops, out, tag, sentAt) {
  const { inputs, refersTo } = messageFor(channel, cellId, gram);
  const to = channel.to.cell;
  let grams;
  try {
    grams = receive(to, channel.to.article, refersTo, inputs, sentAt) ?? [];
  } catch (e) {
    if (e?.name === NOT_ADDRESSED) return false;
    if (e?.name !== ANSWERED) {
      out.undelivered.push({ cellId, gramId: gram.id, channel: channelKey(channel), error: String(e?.message ?? e), tag });
    }
    return true;
  }
  for (const g of grams) {
    out.recorded.push({ cellId: to, gram: g });
    send(g, to, channels, receive, hops + 1, out, tag);
  }
  return true;
}
