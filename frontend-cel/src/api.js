// De routes van de runtime, van een cel en van een proces. Elke fout komt
// terug als {fout: "..."}; de gedeelde apiFetch doet de ok-check en gooit een
// ApiError met die tekst als message en de HTTP-status als `status`.
import { apiFetch } from '@regelrecht/frontend-shared/apiFetch.js';

// De tekst onder `fout` in een foutantwoord, anders de HTTP-status.
export function foutTekst(status, body) {
  try {
    const fout = JSON.parse(body)?.fout;
    if (typeof fout === 'string' && fout) return fout;
  } catch {
    // Geen JSON: dan zegt de status het.
  }
  return `HTTP ${status}`;
}

async function vraag(methode, pad, body) {
  const resp = await apiFetch(pad, {
    method: methode,
    credentials: 'same-origin',
    headers: body ? { 'content-type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
    errorMessage: foutTekst,
  });
  return resp.status === 204 ? null : resp.json();
}

// De cellen van de runtime, met per cel haar kronieken en lexostatussen.
export const cellen = () => vraag('GET', '/api/cellen');

// De processen van de runtime, met per proces zijn cel en mogelijkheden.
export const processen = () => vraag('GET', '/api/processen');

// De routes van een cel, onder /cellen/<id>: wat ze vastlegde en wat haar
// reducties opleveren. Zonder login.
export function celApi(id) {
  const p = `/cellen/${encodeURIComponent(id)}/api`;
  return {
    kroniek: () => vraag('GET', `${p}/kroniek`),
    lexostatus: (naam, invoer) =>
      vraag('GET', `${p}/lexostatus/${encodeURIComponent(naam)}?${new URLSearchParams(invoer)}`),
  };
}

// De routes van een proces, onder /processen/<id>.
export function procesApi(id) {
  const p = `/processen/${encodeURIComponent(id)}/api`;
  return {
    // Inloggen langs een kanaal uit `kanalen` in proces.yaml: de velden van
    // het kanaal, en `rol` als er langs het kanaal meer dan een rol inlogt.
    inloggen: (kanaal, invoer) => vraag('POST', `${p}/kanalen/${encodeURIComponent(kanaal)}/login`, invoer),
    // Wie er is ingelogd, langs welk kanaal ook.
    sessie: () => vraag('GET', `${p}/sessie`),
    uitloggen: (kanaal) => vraag('POST', `${p}/kanalen/${encodeURIComponent(kanaal)}/logout`),
    formulier: () => vraag('GET', `${p}/formulier`),
    toets: (external) => vraag('POST', `${p}/aanvraag/toets`, { external }),
    indienen: (external) => vraag('POST', `${p}/aanvraag`, { external }),
    mogelijkheden: () => vraag('GET', `${p}/mogelijkheden`),
    // Standaardgegevens per handeling; ook zonder login.
    voorbeelden: () => vraag('GET', `${p}/voorbeelden`),
    // Het loket: een aanvraag die langs een andere weg binnenkwam,
    // {aanvrager, ontvangen_op, external}.
    loketIndienen: (invoer) => vraag('POST', `${p}/loket/aanvraag`, invoer),
    werkvoorraad: () => vraag('GET', `${p}/werkvoorraad`),
    zaak: (zaakkenmerk) => vraag('GET', `${p}/zaken/${encodeURIComponent(zaakkenmerk)}`),
    proefbesluit: (zaakkenmerk, formulier) =>
      vraag('POST', `${p}/zaken/${encodeURIComponent(zaakkenmerk)}/proefbesluit`, { formulier }),
    besluit: (zaakkenmerk, formulier) =>
      vraag('POST', `${p}/zaken/${encodeURIComponent(zaakkenmerk)}/besluit`, { formulier }),
  };
}
