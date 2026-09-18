/**
 * useAssistent - praat met de beleidsassistent-backend (packages/poc-assistent,
 * poort 3600, via de vite dev-proxy op /api).
 *
 * Een gesprek en niet een losse opdracht: `run()` opent de SSE-stream en houdt
 * die open, en zolang hij openstaat kan er via `stuur()` een bericht bij en via
 * `antwoord()` een keuze terug. Beide gaan naar hetzelfde CLI-proces, dus de
 * assistent onthoudt wat er eerder is gezegd.
 *
 * De /api/assistent-respons is een POST-SSE-stream (text/event-stream). Een
 * EventSource kan geen POST doen, dus we lezen de ReadableStream zelf en
 * parsen 'data:'-regels als JSON. Berichttypes (contract van de backend):
 *   {type:'gesprek', gesprek_id}       eerst; nodig voor stuur/antwoord
 *   {type:'tekst', tekst}              complete beurt
 *   {type:'tekst_deel', tekst}         hetzelfde, terwijl het getypt wordt
 *   {type:'gebruiker', tekst}          wat wij insturen, voor de feed
 *   {type:'voortgang', beurten, seconden, status, tool}
 *   {type:'vraag', id, vraag, opties, meerkeuze?, toelichting?}
 *   {type:'vraag_verlopen', id}
 *   {type:'tool', naam, input}
 *   {type:'wijziging', document_key, toelichting}
 *   {type:'simulatie', doel, n?, metrics?, wijzigingen?}
 *   {type:'klaar', overlays: {document_key: yaml}, beurten, seconden}
 *   {type:'fout', melding}
 */
import { ref } from 'vue';
import { b } from '../basePad.js';

export function useAssistent() {
  const available = ref(null); // null = onbekend, true/false na health-check
  const model = ref(null);
  // Kan deze omgeving een variant opslaan? Hosted niet: dat maakt een
  // git-branch in de casus-checkout, en die is er in de container niet.
  const variantOpslag = ref(false);
  const streaming = ref(false);

  async function checkHealth() {
    try {
      const res = await fetch(b('/api/health'), { signal: AbortSignal.timeout(3000) });
      if (!res.ok) throw new Error(String(res.status));
      const data = await res.json();
      available.value = !!data.ok;
      model.value = data.model ?? null;
      variantOpslag.value = data.varianten === true;
    } catch {
      available.value = false;
    }
    return available.value;
  }

  /**
   * Start een assistent-run. onEvent krijgt elk geparst SSE-bericht.
   * @returns {Promise<void>} rondt af als de stream sluit.
   */
  let controller = null;

  /**
   * Het id van het lopende gesprek, zodra de backend het heeft gestuurd.
   * Nodig om een vervolgbericht of het antwoord op een keuze te versturen.
   */
  const gesprekId = ref(null);

  /** Breek de lopende run af; de backend stopt dan het CLI-proces. */
  function abort() {
    controller?.abort();
  }

  /**
   * Stuur een bericht naar het lopende gesprek. De assistent pakt het bij zijn
   * volgende beurt op, ook als hij nu nog aan het rekenen is: dat is precies
   * het punt van een gesprek in plaats van losse opdrachten.
   */
  async function stuur(tekst) {
    if (!gesprekId.value) throw new Error('Er loopt geen gesprek.');
    const res = await fetch(b(`/api/gesprek/${gesprekId.value}/bericht`), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tekst }),
    });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).fout ?? `Backend gaf ${res.status}`);
  }

  /** Beantwoord een keuze die de assistent heeft voorgelegd. */
  async function antwoord(vraagId, keuzes) {
    if (!gesprekId.value) throw new Error('Er loopt geen gesprek.');
    const res = await fetch(b(`/api/gesprek/${gesprekId.value}/antwoord`), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ vraag_id: vraagId, keuzes }),
    });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).fout ?? `Backend gaf ${res.status}`);
  }

  async function run({ modus, prompt, documenten = [] }, onEvent) {
    streaming.value = true;
    controller = new AbortController();
    try {
      const res = await fetch(b('/api/assistent'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ modus, prompt, documenten }),
        signal: controller.signal,
      });
      if (!res.ok) {
        // 503 = te veel gesprekken tegelijk; die melding is voor de gebruiker.
        const body = await res.json().catch(() => ({}));
        throw new Error(body.fout ?? `Backend gaf ${res.status}`);
      }
      if (!res.body) throw new Error('De backend stuurde geen stream.');

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = '';

      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        // SSE-berichten zijn gescheiden door een lege regel.
        const parts = buffer.split('\n\n');
        buffer = parts.pop() ?? '';
        for (const part of parts) {
          for (const line of part.split('\n')) {
            const trimmed = line.trimStart();
            if (!trimmed.startsWith('data:')) continue;
            const payload = trimmed.slice(5).trim();
            if (!payload) continue;
            try {
              const ev = JSON.parse(payload);
              if (ev.type === 'gesprek') gesprekId.value = ev.gesprek_id ?? null;
              onEvent(ev);
            } catch {
              // ongeldige regel overslaan
            }
          }
        }
      }
    } catch (e) {
      if (e?.name !== 'AbortError') onEvent({ type: 'fout', melding: String(e?.message ?? e) });
    } finally {
      streaming.value = false;
      gesprekId.value = null;
    }
  }

  return { available, model, variantOpslag, streaming, gesprekId, checkHealth, run, stuur, antwoord, abort };
}
