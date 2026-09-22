// De routes van de runtime en van een cel. Elke fout komt terug als
// {fout: "..."}.

async function vraag(methode, pad, body) {
  const resp = await fetch(pad, {
    method: methode,
    credentials: 'same-origin',
    headers: body ? { 'content-type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  if (resp.status === 204) return null;
  const data = await resp.json().catch(() => null);
  if (!resp.ok) {
    const fout = new Error(data?.fout ?? `${resp.status} ${resp.statusText}`);
    fout.status = resp.status;
    throw fout;
  }
  return data;
}

// De cellen van de runtime, met per cel haar mogelijkheden.
export const cellen = () => vraag('GET', '/api/cellen');

// De routes van een cel, onder /cellen/<id>.
export function celApi(id) {
  const p = `/cellen/${encodeURIComponent(id)}/api`;
  return {
    inloggen: (login) => vraag('POST', `${p}/eherkenning/login`, login),
    sessie: () => vraag('GET', `${p}/eherkenning/sessie`),
    uitloggen: () => vraag('POST', `${p}/eherkenning/logout`),
    stroom: () => vraag('GET', `${p}/stroom`),
    toets: (external) => vraag('POST', `${p}/aanvraag/toets`, { external }),
    indienen: (external) => vraag('POST', `${p}/aanvraag`, { external }),
    kroniek: () => vraag('GET', `${p}/kroniek`),
    medewerkerInloggen: (naam) => vraag('POST', `${p}/medewerker/login`, { naam }),
    medewerkerSessie: () => vraag('GET', `${p}/medewerker/sessie`),
    medewerkerUitloggen: () => vraag('POST', `${p}/medewerker/logout`),
    werkvoorraad: () => vraag('GET', `${p}/werkvoorraad`),
    zaak: (zaakkenmerk) => vraag('GET', `${p}/zaken/${encodeURIComponent(zaakkenmerk)}`),
    proefbesluit: (zaakkenmerk, formulier) =>
      vraag('POST', `${p}/zaken/${encodeURIComponent(zaakkenmerk)}/proefbesluit`, { formulier }),
    lexostatus: (naam, invoer) =>
      vraag('GET', `${p}/lexostatus/${encodeURIComponent(naam)}?${new URLSearchParams(invoer)}`),
  };
}
