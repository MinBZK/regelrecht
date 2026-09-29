// De routes van de runtime, van een cel en van een proces. Elke fout komt
// terug als {error: "..."}; de gedeelde apiFetch doet de ok-check en gooit een
// ApiError met die tekst als message en de HTTP-status als `status`.
import { apiFetch } from '@regelrecht/frontend-shared/apiFetch.js';

// De tekst onder `error` in een foutantwoord, anders de HTTP-status.
export function foutTekst(status, body) {
  try {
    const fout = JSON.parse(body)?.error;
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
export const cellen = () => vraag('GET', '/api/cells');

// De processen van de runtime, met per proces zijn cel en mogelijkheden.
export const processen = () => vraag('GET', '/api/processes');

// Wat een cel vastlegde en wat haar reducties opleveren. De leesroutes van
// een cel zijn niet open (de grammen dragen de identiteit van wie indiende);
// een behandelaar ziet ze via zijn proces, onder
// /processes/<proces>/api/inspection/<cel>.
export function inzageApi(proces, cel) {
  const p = `/processes/${encodeURIComponent(proces)}/api/inspection/${encodeURIComponent(cel)}`;
  return {
    kroniek: () => vraag('GET', `${p}/chronicle`),
    lexostatus: (naam, invoer) =>
      vraag('GET', `${p}/lexostatus/${encodeURIComponent(naam)}?${new URLSearchParams(invoer)}`),
  };
}

// De routes van een proces, onder /processes/<id>.
export function procesApi(id) {
  const p = `/processes/${encodeURIComponent(id)}/api`;
  const handeling = (z, naam) => `${p}/cases/${encodeURIComponent(z)}/actions/${encodeURIComponent(naam)}`;
  return {
    // Inloggen langs een kanaal uit `channels` in process.yaml: de velden van
    // het kanaal, en `role` als er langs het kanaal meer dan een rol inlogt.
    inloggen: (kanaal, invoer) => vraag('POST', `${p}/channels/${encodeURIComponent(kanaal)}/login`, invoer),
    // Wie er is ingelogd, langs welk kanaal ook.
    sessie: () => vraag('GET', `${p}/session`),
    uitloggen: (kanaal) => vraag('POST', `${p}/channels/${encodeURIComponent(kanaal)}/logout`),
    formulier: () => vraag('GET', `${p}/form`),
    toets: (external) => vraag('POST', `${p}/application/assessment`, { external }),
    indienen: (external) => vraag('POST', `${p}/application`, { external }),
    mogelijkheden: () => vraag('GET', `${p}/possibilities`),
    // Standaardgegevens per handeling; ook zonder login.
    voorbeelden: () => vraag('GET', `${p}/examples`),
    // Het loket: een aanvraag die langs een andere weg binnenkwam,
    // {applicant, received_at, external}.
    loketIndienen: (invoer) => vraag('POST', `${p}/counter/application`, invoer),
    werkvoorraad: () => vraag('GET', `${p}/worklist`),
    zaak: (wortel) => vraag('GET', `${p}/cases/${encodeURIComponent(wortel)}`),
    // Een handeling in een zaak (het besluit, een latere stage, een feit uit
    // het verloop): op proef, of genomen en vastgelegd. Een route voor elke
    // handeling; welke er zijn, zegt de zaak.
    proefhandeling: (wortel, naam, formulier) =>
      vraag('POST', `${handeling(wortel, naam)}/trial`, { form: formulier }),
    // Met `happened` meldt de behandelaar een feit dat gebeurde terwijl de
    // proef om de inhoud nee zei.
    handeling: (wortel, naam, formulier, gebeurd = false) =>
      vraag('POST', handeling(wortel, naam), gebeurd ? { form: formulier, happened: gebeurd } : { form: formulier }),
  };
}
