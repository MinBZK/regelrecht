// De routes van de cel. Elke fout komt terug als {fout: "..."}.

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

export const api = {
  inloggen: (login) => vraag('POST', '/api/eherkenning/login', login),
  sessie: () => vraag('GET', '/api/eherkenning/sessie'),
  uitloggen: () => vraag('POST', '/api/eherkenning/logout'),
  stroom: () => vraag('GET', '/api/stroom'),
  toets: (external) => vraag('POST', '/api/aanvraag/toets', { external }),
  indienen: (external) => vraag('POST', '/api/aanvraag', { external }),
  kroniek: () => vraag('GET', '/api/kroniek'),
};
