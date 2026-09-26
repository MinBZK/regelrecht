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

// Wat een cel vastlegde en wat haar reducties opleveren. De leesroutes van
// een cel zijn niet open (de grammen dragen de identiteit van wie indiende);
// een behandelaar ziet ze via zijn proces, onder
// /processen/<proces>/api/inzage/<cel>.
export function inzageApi(proces, cel) {
  const p = `/processen/${encodeURIComponent(proces)}/api/inzage/${encodeURIComponent(cel)}`;
  return {
    kroniek: () => vraag('GET', `${p}/kroniek`),
    lexostatus: (naam, invoer) =>
      vraag('GET', `${p}/lexostatus/${encodeURIComponent(naam)}?${new URLSearchParams(invoer)}`),
  };
}

// De routes van een proces, onder /processen/<id>.
export function procesApi(id) {
  const p = `/processen/${encodeURIComponent(id)}/api`;
  const handeling = (z, naam) => `${p}/zaken/${encodeURIComponent(z)}/handelingen/${encodeURIComponent(naam)}`;
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
    // Een handeling in een zaak (het besluit, een latere stage, een feit uit
    // het verloop): op proef, of genomen en vastgelegd. Een route voor elke
    // handeling; welke er zijn, zegt de zaak.
    proefhandeling: (zaakkenmerk, naam, formulier) =>
      vraag('POST', `${handeling(zaakkenmerk, naam)}/proef`, { formulier }),
    // Met `gebeurd` meldt de behandelaar een feit dat gebeurde terwijl de
    // proef om de inhoud nee zei.
    handeling: (zaakkenmerk, naam, formulier, gebeurd = false) =>
      vraag('POST', handeling(zaakkenmerk, naam), gebeurd ? { formulier, gebeurd } : { formulier }),
  };
}
