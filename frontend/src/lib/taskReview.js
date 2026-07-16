/**
 * reviewTarget - de router-target voor de "Beoordelen"-knop van een
 * job_review-taak. Vertakt op `payload.kind`:
 *  - `kind === 'document'`: de werkdocumenten-route (router.js, route
 *    `werkdocumenten-traject`: `trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/
 *    werkdocumenten/:docPath(.*)?`), met `docPath` op `payload.target_path` -
 *    het werkdocument bestaat op de branch meestal nog niet, dus deze route
 *    is ook de deep-link naar een nog-niet-bestaand document.
 *  - anders (wet-review): de traject-scoped editor-route (route
 *    `editor-traject`: `editor/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/
 *    :articleNumber?`), met `payload.law_id`.
 * Beide dragen de taak-id als `?task=`-query zodat de bestemming de taak kan
 * koppelen aan wat er geopend wordt.
 *
 * Puur en los van de router/sheet zodat de route-opbouw te testen is zonder
 * een component te mounten. Geeft `null` terug wanneer de taak niet genoeg
 * payload-velden heeft voor een van beide routes (bijv. een corrupte of
 * onvolledige taak) - de aanroeper toont dan geen/een disabled knop in
 * plaats van te crashen of naar een kapotte route te navigeren.
 */
export function reviewTarget(task) {
  const trajectRef = task?.payload?.traject_ref;
  if (!trajectRef) return null;
  if (task?.payload?.kind === 'document') {
    const targetPath = task.payload.target_path;
    if (!targetPath) return null;
    return {
      name: 'werkdocumenten-traject',
      params: { trajectRef, docPath: targetPath },
      query: { task: task.id },
    };
  }
  const lawId = task?.payload?.law_id;
  if (!lawId) return null;
  return {
    name: 'editor-traject',
    params: { trajectRef, lawId },
    query: { task: task.id },
  };
}

function articleDiffers(current, proposedArticle) {
  const sameText = (current?.text ?? '') === (proposedArticle.text ?? '');
  const sameMr =
    JSON.stringify(current?.machine_readable ?? null) ===
    JSON.stringify(proposedArticle.machine_readable ?? null);
  return !(sameText && sameMr);
}

/**
 * proposalDivergence - vergelijkt de artikelen van de huidige (opgeslagen)
 * wet met de voorgestelde artikelen uit een job_review-taak.
 *
 * Levert het EERSTE artikel dat inhoudelijk afwijkt en dat de editor kan
 * seeden (`target`, `null` als niets seedbaar afwijkt), plus `hiddenChanges`:
 * waar of niet er wijzigingen zijn die de (single-article-scoped) editor
 * niet zichtbaar kan maken. Dat geldt voor:
 *  - een tweede (of latere) afwijkend artikel naast `target`;
 *  - een voorgesteld artikel dat de huidige wet niet heeft (v1 kan alleen
 *    een BESTAAND artikel seeden als unsaved edit, zie applyProposedContent
 *    in EditorView.vue);
 *  - een artikel van de huidige wet dat in het voorstel ONTBREEKT (een
 *    verwijdering) - Opslaan committeert die verwijdering net zo goed als
 *    de rest van het voorstel, dus de banner moet ernaar verwijzen ook al
 *    is er geen "proposed article" om te seeden of te tonen.
 */
export function proposalDivergence(currentArticles, proposedArticles) {
  const current = Array.isArray(currentArticles) ? currentArticles : [];
  const proposed = Array.isArray(proposedArticles) ? proposedArticles : [];

  let target = null;
  let hiddenChanges = false;
  for (const pa of proposed) {
    const match = current.find((a) => String(a.number) === String(pa.number));
    if (!match) {
      hiddenChanges = true;
      continue;
    }
    if (!articleDiffers(match, pa)) continue;
    if (!target) {
      target = pa;
    } else {
      hiddenChanges = true;
    }
  }

  const proposedNumbers = new Set(proposed.map((pa) => String(pa.number)));
  if (current.some((a) => !proposedNumbers.has(String(a.number)))) {
    hiddenChanges = true;
  }

  return { target, hiddenChanges };
}
