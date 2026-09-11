/**
 * useAssistent - praat met de beleidsassistent-backend (server/, poort 3600,
 * via de vite dev-proxy op /api).
 *
 * De /api/assistent-respons is een POST-SSE-stream (text/event-stream). Een
 * EventSource kan geen POST doen, dus we lezen de ReadableStream zelf en
 * parsen 'data:'-regels als JSON. Berichttypes (contract van de backend):
 *   {type:'tekst', tekst}
 *   {type:'tool', naam, input}
 *   {type:'wijziging', document_key, toelichting}
 *   {type:'simulatie', doel, n?, metrics?}
 *   {type:'klaar', overlays: {document_key: yaml}}
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

  /** Breek de lopende run af; de backend stopt dan het CLI-proces. */
  function abort() {
    controller?.abort();
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
      if (!res.ok || !res.body) throw new Error(`Backend gaf ${res.status}`);

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
              onEvent(JSON.parse(payload));
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
    }
  }

  return { available, model, variantOpslag, streaming, checkHealth, run, abort };
}
