<script setup>
/**
 * TasksListPane - de takenlijst zelf, in de main-pane van Home. Toont de
 * categorie die in het panel (TasksCategoriesPane) gekozen is; de route draagt
 * die keuze, dit component leest 'm als props.
 *
 * Lopende jobs staan onderaan elke lijst waar ze in thuishoren - onder je eigen
 * taken, want daar kun jij wél iets mee. Ze zijn technisch geen taak, maar ze
 * liggen wel op je bord: "alle" hoort niet selectief te zijn, en een conversie
 * ván een werkdocument hoort net zo goed bij die context.
 *
 * Elke rij heeft één Acties-menu in plaats van losse knoppen: op mobiel vreten
 * knoppen per rij de breedte op, en zo blijft elke rij even breed ongeacht
 * hoeveel acties eronder zitten.
 *
 * De acties die neveneffecten hebben met eigen foutafhandeling (de
 * bestandskiezer, een conversie annuleren) worden geëmit; LibraryView bezit die
 * bedrading al, inclusief de foutmodals. De rest - navigeren, afhandelen,
 * opnieuw aanvragen - gebeurt hier.
 *
 * Mounten start via `useTasks()` de gedeelde 30s-poll; de route erachter vereist
 * login, dus anonieme bezoekers pollen nooit.
 */
import { computed, nextTick, ref, toRef, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useTasks } from '../composables/useTasks.js';
import { useCorpusLaws } from '../composables/useCorpusLaws.js';
import { homeTarget } from '../composables/useLastVisitedRoute.js';
import { reviewTarget } from '../lib/taskReview.js';
import { taskTitle, runningTitle } from '../lib/taskTitle.js';
import {
  filterTasks,
  filterRunning,
  isPrioriteit,
  taskContext,
  taskLawId,
  jobContext,
  jobLawId,
  WACHTEN,
  WERKDOCUMENTEN,
} from '../lib/taskCategories.js';

const props = defineProps({
  trajectRef: { type: String, default: null },
  categorie: { type: String, default: null },
  lawId: { type: String, default: null },
});

// `upload` opent de bestandskiezer, `cancel-job` annuleert een lopende
// conversie, `view-job` opent de job-weergave van een lopende conversie -
// alle drie van LibraryView, die viewingJobPath + de foutmodals bezit.
const emit = defineEmits(['upload', 'cancel-job', 'view-job']);

const router = useRouter();
const { tasks, running, resolveTask, requestEnrich, refresh } = useTasks();
const { displayName } = useCorpusLaws(toRef(props, 'trajectRef'));

// Mislukt bovenaan, de rest op de servervolgorde (nieuwste eerst). Voorlopig:
// een mislukte taak is de enige die echt vastzit, dus die hoort niet onder een
// scroll. Een echte prioritering (deadline, ouderdom) komt later.
const shownTasks = computed(() => {
  const list = filterTasks(tasks.value, props.categorie, props.lawId);
  const failed = list.filter(isPrioriteit);
  return failed.length ? [...failed, ...list.filter((t) => !isPrioriteit(t))] : list;
});

const shownRunning = computed(() => filterRunning(running.value, props.categorie, props.lawId));

const isEmpty = computed(() => shownTasks.value.length === 0 && shownRunning.value.length === 0);

// Menu-ids: een taak-id is een UUID en kan met een cijfer beginnen, wat een
// ongeldige CSS-selector oplevert voor de anchor-lookup. Het prefix houdt 'm
// geldig.
const menuId = (id) => `taak-acties-menu-${id}`;
const btnId = (id) => `taak-acties-${id}`;

function dismiss(task) {
  resolveTask(task.id, 'dismissed');
}

// --- Taakdetails-sheet ---
// De rauwe foutmelding hoort niet in de rij: het is een technische string van
// soms tweehonderd tekens ("no push token for traject source '…' (expected env
// CORPUS_AUTH_…)"), en de app toont 'm nergens anders - de job-weergave heeft er
// een mensentekst voor. Achter "Toon details" staat hij wel, want dat is de plek
// waar je 'm komt halen.
//
// Eén sheet voor de hele lijst, niet één per rij; `detailTask` bepaalt de
// inhoud. nldd-sheet heeft imperatieve show()/hide(), dus die spiegelen we
// vanuit een watcher (zelfde patroon als TrajectMenu).
const detailSheetEl = ref(null);
const detailTask = ref(null);
watch(detailTask, async (task) => {
  await nextTick();
  if (task) detailSheetEl.value?.show();
  else detailSheetEl.value?.hide();
});
function showDetails(task) {
  detailTask.value = task;
}
function closeDetails() {
  detailTask.value = null;
}

// "Probeer opnieuw" op een mislukte taak. Twee mechanieken achter één
// bedoeling: een conversie kan alleen opnieuw via een nieuwe upload (de job
// draait met max_attempts=1, bewust - zie corpus_handlers.rs), een verrijking
// vraag je gewoon opnieuw aan.
async function retry(task) {
  if (taskContext(task) === WERKDOCUMENTEN) {
    emit('upload');
    return;
  }
  const lawId = taskLawId(task);
  if (!lawId || !props.trajectRef) return;
  await requestEnrich(props.trajectRef, lawId);
  await refresh();
}

// Twee iconen, om de actie: een gevulde uitroep-cirkel = er ging iets stuk
// (zelfde cirkel-familie als de Prioriteit-ingang, die de open variant draagt),
// 'eyeglasses' = hier moet jij naar kijken en beoordelen. Het onderwerp
// (werkdocument vs wet) zegt de rij zelf al - via de titel en via de context
// waar hij onder staat - dus het icoon hoeft dat niet te herhalen.
function taskIcon(task) {
  return task.task_type === 'job_failed' ? 'exclamation-circle-filled' : 'eyeglasses';
}

// "Beoordelen" navigeert naar het artikel in de editor (met de taak-id als
// query). Taken zonder traject_ref/law_id in de payload tonen een disabled
// menu-item (zie :disabled hieronder) in plaats van te crashen op een
// onvolledige route.
function review(task) {
  const target = reviewTarget(task);
  if (!target) return;
  router.push(target);
}

// "Bekijk document" op een lopende conversie: de .md bestaat nog niet, dus we
// willen de job-weergave ("Aan het converteren…"), niet het document. Via een
// emit i.p.v. router.push, zodat LibraryView viewingJobPath vóór de navigatie
// zet - anders opent hij het pad eerst als gewoon document (de jobs-lijst is
// bij binnenkomst nog niet gepolld) en flitst de 404/‘mislukt’-staat op. Zo
// landt het deterministisch op de job-weergave, net als de zijbalk.
function viewDocument(job) {
  if (!job.target_path) return;
  emit('view-job', job.target_path);
}

// "Bekijk wet" op een lopende verrijking: annuleren kan niet (die endpoint
// bestaat alleen voor conversies), maar de wet zelf bestaat wel - dus open 'm
// in de bibliotheek. Zonder dit zou een lopende verrijking een leeg menu
// hebben.
function viewLaw(job) {
  const lawId = jobLawId(job);
  if (!lawId) return;
  router.push(homeTarget({ trajectRef: props.trajectRef || undefined, lawId }));
}
</script>

<template>
  <nldd-list v-if="!isEmpty" variant="simple">
    <nldd-list-item v-for="task in shownTasks" :key="task.id" size="md">
      <nldd-icon-cell
        slot="start"
        size="20"
        :icon="taskIcon(task)"
        :color="task.task_type === 'job_failed' ? 'critical' : undefined"
      ></nldd-icon-cell>
      <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
      <!-- Geen supporting-text met de foutmelding: die is technisch en lang, en
           duwt de rij uit z'n voegen. Hij staat achter "Toon details". -->
      <nldd-text-cell
        :text="taskTitle(task, displayName)"
        :color="task.task_type === 'job_failed' ? 'critical' : undefined"
      ></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-cell>
        <!-- Bewust géén `expandable`: die zet nog een chevron náást het icoon,
             en een overflow-knop die al "meer" zegt heeft geen tweede
             open-signaal nodig. `popup-type` levert de aria-haspopup/-expanded
             die `expandable` anders zou geven. -->
        <nldd-icon-button
          :id="btnId(task.id)"
          size="sm"
          icon="more"
          text="Acties"
          tooltip-timing="never"
          popup-type="menu"
          :popovertarget="menuId(task.id)"
        ></nldd-icon-button>
        <nldd-menu :id="menuId(task.id)" :anchor="btnId(task.id)">
          <!-- Details bovenaan: bij een mislukking wil je eerst weten wát er
               misging voordat je 'm opnieuw probeert of wegzet. Kijken staat
               dus los van de twee acties die er iets mee doen. -->
          <template v-if="task.task_type === 'job_failed'">
            <nldd-menu-item text="Toon details" @select="showDetails(task)"></nldd-menu-item>
            <nldd-menu-divider></nldd-menu-divider>
            <nldd-menu-item text="Probeer opnieuw" @select="retry(task)"></nldd-menu-item>
          </template>
          <template v-else>
            <nldd-menu-item
              text="Beoordelen"
              :disabled="!reviewTarget(task) || undefined"
              @select="review(task)"
            ></nldd-menu-item>
          </template>
          <nldd-menu-divider></nldd-menu-divider>
          <!-- "Markeer als gedaan" is `dismissed`: van je lijst af, zonder
               oordeel. `rejected` (het voorstel deugt niet) hoort in de
               review-UI - daar zie je wat je verwerpt. -->
          <nldd-menu-item text="Markeer als gedaan" @select="dismiss(task)"></nldd-menu-item>
        </nldd-menu>
      </nldd-cell>
    </nldd-list-item>

    <nldd-list-item v-for="job in shownRunning" :key="job.job_id" size="md">
      <!-- Een gewone cell, geen icon-cell: een activity-indicator is geen icoon
           en heeft de icon-schaal van die cell niet nodig.
           timing="instant": de anti-flash-vertraging van 1000ms is bedoeld voor
           laadjes die zo weer weg zijn. Deze staan er minutenlang, dus die
           vertraging levert alleen een gat op waar de rij al zichtbaar is. -->
      <nldd-cell slot="start" vertical-alignment="center">
        <nldd-activity-indicator
          size="20"
          timing="instant"
          :text="runningTitle(job, displayName)"
        ></nldd-activity-indicator>
      </nldd-cell>
      <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
      <nldd-text-cell :text="runningTitle(job, displayName)"></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-cell>
        <nldd-icon-button
          :id="btnId(job.job_id)"
          size="sm"
          icon="more"
          text="Acties"
          tooltip-timing="never"
          popup-type="menu"
          :popovertarget="menuId(job.job_id)"
        ></nldd-icon-button>
        <nldd-menu :id="menuId(job.job_id)" :anchor="btnId(job.job_id)">
          <template v-if="jobContext(job) === WERKDOCUMENTEN">
            <nldd-menu-item text="Bekijk document" @select="viewDocument(job)"></nldd-menu-item>
            <!-- Annuleren gooit de job én de bron-upload weg, dus het staat los
                 van het onschuldige kijken erboven, en het is destructive. -->
            <nldd-menu-divider></nldd-menu-divider>
            <nldd-menu-item
              variant="destructive"
              text="Annuleer conversie"
              @select="emit('cancel-job', job)"
            ></nldd-menu-item>
          </template>
          <nldd-menu-item v-else text="Bekijk wet" @select="viewLaw(job)"></nldd-menu-item>
        </nldd-menu>
      </nldd-cell>
    </nldd-list-item>
  </nldd-list>

  <!-- De lege staat verschilt per categorie: bij "Wachten op" loopt er niets,
       elders ligt er niets. -->
  <nldd-inline-dialog
    v-else-if="categorie === WACHTEN"
    text="Niets loopt op dit moment."
    supporting-text="Een verrijking of conversie die je aanvraagt verschijnt hier tot hij klaar is."
  ></nldd-inline-dialog>
  <nldd-inline-dialog v-else text="Geen taken"></nldd-inline-dialog>

  <!-- Teleport naar body: deze lijst hangt in een split-view-pane, en een sheet
       die daar als broer blijft staan pikt via ::slotted flex-grow de
       pane-hoogte in. Zelfde reden als de create-sheet in TrajectMenu. -->
  <Teleport to="body">
    <nldd-sheet
      ref="detailSheetEl"
      placement="right"
      width="520px"
      full-height
      @close="closeDetails"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Taakdetails"
          dismiss-text="Sluit"
          @dismiss="closeDetails"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="detailTask">
          <nldd-title size="3"><h3>{{ taskTitle(detailTask, displayName) }}</h3></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-rich-text v-if="detailTask.payload?.error">
            <h4>Foutmelding</h4>
            <p>{{ detailTask.payload.error }}</p>
          </nldd-rich-text>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
