/**
 * reviewTarget - de router-target voor de "Beoordelen"-knop van een
 * job_review-taak: de traject-scoped editor-route voor de wet (zie
 * router.js, route `editor-traject`:
 * `editor/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?`),
 * met de taak-id als `?task=`-query zodat de editor de taak kan koppelen aan
 * het geopende artikel.
 *
 * Puur en los van de router/sheet zodat de route-opbouw te testen is zonder
 * een component te mounten. Geeft `null` terug wanneer de taak geen
 * `traject_ref`/`law_id` in de payload heeft (bijv. een corrupte of
 * onvolledige taak) - de aanroeper toont dan geen/een disabled knop in
 * plaats van te crashen of naar een kapotte route te navigeren.
 */
export function reviewTarget(task) {
  const trajectRef = task?.payload?.traject_ref;
  const lawId = task?.payload?.law_id;
  if (!trajectRef || !lawId) return null;
  return {
    name: 'editor-traject',
    params: { trajectRef, lawId },
    query: { task: task.id },
  };
}
