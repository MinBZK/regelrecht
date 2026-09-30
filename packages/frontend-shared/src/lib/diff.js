/**
 * Minimale regel-diff (LCS) voor de wetswijziging-sheet. Geen dependency;
 * genoeg om toegevoegde/verwijderde regels tussen twee YAML-teksten te tonen.
 */

/** @returns {Array<{type:'context'|'add'|'del', text:string}>} */
export function lineDiff(a, b) {
  const A = a.split('\n');
  const B = b.split('\n');
  const n = A.length;
  const m = B.length;

  // LCS-lengtes.
  const dp = Array.from({ length: n + 1 }, () => new Int32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] = A[i] === B[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const out = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (A[i] === B[j]) {
      out.push({ type: 'context', text: A[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ type: 'del', text: A[i] });
      i++;
    } else {
      out.push({ type: 'add', text: B[j] });
      j++;
    }
  }
  while (i < n) out.push({ type: 'del', text: A[i++] });
  while (j < m) out.push({ type: 'add', text: B[j++] });
  return out;
}

/** Alleen de gewijzigde blokken (context ingekort tot 2 regels rondom). */
export function compactDiff(a, b, contextLines = 2) {
  const full = lineDiff(a, b);
  const keep = new Array(full.length).fill(false);
  full.forEach((d, idx) => {
    if (d.type !== 'context') {
      for (let k = Math.max(0, idx - contextLines); k <= Math.min(full.length - 1, idx + contextLines); k++) {
        keep[k] = true;
      }
    }
  });
  const out = [];
  let gap = false;
  full.forEach((d, idx) => {
    if (keep[idx]) {
      out.push(d);
      gap = false;
    } else if (!gap) {
      out.push({ type: 'gap', text: '…' });
      gap = true;
    }
  });
  return out;
}

/**
 * Artikelen waarvan de wettekst is gewijzigd, als
 * [{ article, oud, nieuw }].
 *
 * Waarom apart van definitionDiff: een wet is zijn tekst, en een wijziging die
 * alleen in de formule zit vertelt een andere wet dan de proza eromheen. Wie
 * de diff leest, moet beide zien staan.
 */
export function articleTextDiff(baseDoc, currentDoc) {
  const changes = [];
  const curByNumber = new Map((currentDoc?.articles ?? []).map((a) => [String(a.number), a]));
  for (const art of baseDoc?.articles ?? []) {
    const cur = curByNumber.get(String(art.number));
    if (!cur) continue;
    const oud = typeof art.text === 'string' ? art.text : null;
    const nieuw = typeof cur.text === 'string' ? cur.text : null;
    if (oud !== null && nieuw !== null && oud !== nieuw) {
      changes.push({ article: art.number, oud, nieuw });
    }
  }
  return changes;
}

/**
 * Verschillen in definitie-waarden tussen twee geparste documenten, als
 * leesbare "artikel X: naam: oud → nieuw"-regels.
 */
export function definitionDiff(baseDoc, currentDoc) {
  const changes = [];
  const baseArticles = baseDoc?.articles ?? [];
  const curArticles = currentDoc?.articles ?? [];
  const curByNumber = new Map(curArticles.map((a) => [String(a.number), a]));

  for (const art of baseArticles) {
    const baseDefs = art?.machine_readable?.definitions;
    if (!baseDefs) continue;
    const curArt = curByNumber.get(String(art.number));
    const curDefs = curArt?.machine_readable?.definitions ?? {};
    for (const [name, def] of Object.entries(baseDefs)) {
      const oud = def?.value;
      const nieuw = curDefs[name]?.value;
      if (nieuw !== undefined && JSON.stringify(oud) !== JSON.stringify(nieuw)) {
        changes.push({ article: art.number, name, oud, nieuw });
      }
    }
  }
  return changes;
}
