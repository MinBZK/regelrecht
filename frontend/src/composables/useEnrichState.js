/**
 * useEnrichState - de verrijk-/beoordeel-staat van één wet, zoals de lege
 * Machine- en YAML-panes die tonen: loopt er een verrijking, staat er een
 * voorstel klaar, en waar moet je heen als je erop klikt.
 *
 * Bestaat omdat Bibliotheek en Editor allebei dezelfde wet in dezelfde panes
 * tonen en dus hetzelfde moeten melden. Die blokken stonden regel-voor-regel
 * dubbel en liepen uit elkaar: de ene helft kreeg wél een refresh van de
 * gedeelde takenlijst en de andere niet, waardoor de editor bij een deeplink
 * "Genereer een voorstel" aanbood terwijl er al een voorstel klaarlag.
 *
 * Bewust op useTaskActions gebouwd en niet op useTasks: dit is de niet-pollende
 * helft. Een view die een wet toont mag de 30s-poll van de takenlijst niet
 * ongevraagd starten (die draait ook voor anonieme bezoekers), dus verversen we
 * gericht: één keer bij mount, na elke eigen verrijk-aanvraag, en zolang je naar
 * de bezig-staat kijkt (usePollWhile).
 */
import { computed, onMounted, toValue } from 'vue';
import { useRouter } from 'vue-router';
import { useAuth } from './useAuth.js';
import { useTaskActions, usePollWhile } from './useTasks.js';
import { reviewTarget } from '../lib/taskReview.js';

/**
 * @param {object} sources - refs/getters van de view.
 * @param {import('vue').Ref<string|null>} sources.trajectRef - actief traject.
 * @param {import('vue').Ref<string|null>} sources.lawId - de getoonde wet.
 * @param {import('vue').Ref<string|number|null>} sources.articleNumber - het geopende artikel.
 * @param {import('vue').Ref<boolean>} [sources.reviewActive] - beoordeel je die
 *   taak op dit moment al? Dan hoeft de pane 'er ligt een voorstel klaar' niet
 *   te melden. Alleen de editor kent die modus.
 */
export function useEnrichState({ trajectRef, lawId, articleNumber, reviewActive }) {
  const router = useRouter();
  const { authenticated } = useAuth();
  const {
    refresh: refreshTasks,
    requestEnrich: requestEnrichRaw,
    running: runningJobs,
    tasks: openTasks,
  } = useTaskActions();

  // Loopt er al een verrijking voor deze wet? Dan meldt de lege pane dat in
  // plaats van opnieuw de knoppen aan te bieden; een tweede aanvraag zou toch
  // op een 409 stuiten.
  const isEnriching = computed(() =>
    runningJobs.value.some(
      (job) => job.job_type === 'enrich' && job.law_id === toValue(lawId),
    ),
  );
  // Alleen pollen zolang je naar de bezig-staat kijkt; zie usePollWhile. Staat
  // hier ná isEnriching en achter de argumenten van deze composable: de watch
  // leest zijn bron meteen om de dependency te leggen, dus alles wat hij
  // aanraakt moet al bestaan. Doordat de view zijn refs als argument meegeeft,
  // is die volgorde hier structureel goed in plaats van een valkuil die elke
  // aanroeper zelf moet onthouden.
  usePollWhile(isEnriching);

  // Staat er al een beoordeelbaar voorstel klaar voor deze wet? Eén verrijking
  // levert een taak per gewijzigd artikel, dus meestal staan er meerdere open
  // voor deze wet. Je kijkt naar één artikel, dus de taak van dát artikel gaat
  // voor; is die er niet, dan de eerste van de wet - beter naar een naburig
  // voorstel wijzen dan naar niets.
  const pendingReviewTask = computed(() => {
    const forLaw = openTasks.value.filter(
      (t) => t.task_type === 'job_review' && t.payload?.law_id === toValue(lawId),
    );
    if (!forLaw.length) return null;
    const here = String(toValue(articleNumber) ?? '');
    return forLaw.find((t) => String(t.payload?.article ?? '') === here) ?? forLaw[0];
  });
  const reviewReady = computed(
    () => !!pendingReviewTask.value && !toValue(reviewActive),
  );
  const reviewArticleForPane = computed(() =>
    pendingReviewTask.value?.payload?.article == null
      ? ''
      : String(pendingReviewTask.value.payload.article),
  );

  function openReviewForLaw() {
    const target = reviewTarget(pendingReviewTask.value);
    if (target) router.push(target);
  }

  // Naar de takenlijst, gefilterd op deze wet: daar staat de lopende aanvraag
  // als rij met activity-indicator.
  function openTasksForLaw() {
    const ref_ = toValue(trajectRef);
    const id = toValue(lawId);
    if (!ref_ || !id) return;
    router.push({
      name: 'taken-traject',
      params: { trajectRef: ref_, categorie: 'wet', contextLawId: id },
    });
  }

  // `running`/`tasks` worden alleen door een refresh gevuld; deze composable
  // pollt niet uit zichzelf. Eén keer bij mount is genoeg om na een herlaad of
  // een deeplink te weten wat er voor deze wet loopt of klaarstaat.
  onMounted(() => {
    if (authenticated.value) refreshTasks();
  });

  // Verrijken aanvragen mét de refresh erachteraan: zonder die refresh blijft
  // `isEnriching` false, slaat de bezig-staat niet om en begint usePollWhile
  // nooit te pollen. Die koppeling hoort hier en niet bij elke aanroeper - daar
  // is hij één keer vergeten.
  async function requestEnrich() {
    const result = await requestEnrichRaw(toValue(trajectRef), toValue(lawId));
    await refreshTasks();
    return result;
  }

  return {
    isEnriching,
    pendingReviewTask,
    reviewReady,
    reviewArticleForPane,
    openReviewForLaw,
    openTasksForLaw,
    requestEnrich,
    refreshTasks,
  };
}
