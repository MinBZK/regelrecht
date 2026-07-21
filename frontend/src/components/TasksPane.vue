<script setup>
/**
 * TasksPane - de takenlijst (job_review + job_failed) van de ingelogde
 * gebruiker. Inhoud van het taken-panel in Home: LibraryView toont dit in de
 * secondary sidebar op de taken-route, zoals de documentenlijst in
 * werkdocumenten-modus (voorheen was dit een nldd-sheet achter een
 * topbar-knop). Mounten start via `useTasks()` de gedeelde 30s-poll; de
 * route erachter vereist login, dus anonieme bezoekers pollen nooit.
 */
import { useRouter } from 'vue-router';
import { useTasks } from '../composables/useTasks.js';
import { reviewTarget } from '../lib/taskReview.js';

const router = useRouter();
const { tasks, running, resolveTask } = useTasks();

function dismiss(task) {
  resolveTask(task.id, 'dismissed');
}

// A failed task always gets the alert icon; an open job_review task's icon
// depends on what it's reviewing - a document-conversion proposal
// (payload.kind === 'document') gets the 'documents' alias (icon-aliases.js:
// 'documents' -> file-text-stack, already used elsewhere e.g. TrajectMenu.vue),
// anything else (a law-review proposal) keeps the generic 'tasks' icon.
function taskIcon(task) {
  if (task.task_type === 'job_failed') return 'alert';
  if (task.payload?.kind === 'document') return 'documents';
  return 'tasks';
}

// Label voor een lopende taak-flow-job. Een document_convert-job draagt een
// synthetische `doc:`-sleutel als law_id; de bestandsnaam uit target_path is
// het herkenbare handvat voor de gebruiker.
function runningText(job) {
  if (job.job_type === 'document_convert') {
    const name = job.target_path?.split('/').pop() || 'werkdocument';
    return `Conversie loopt - ${name}`;
  }
  if (job.job_type === 'law_convert') {
    // target_path draagt voor law_convert de geüploade bestandsnaam
    // (COALESCE in list_running_task_jobs_for_account).
    const name = job.target_path?.split('/').pop() || 'document';
    return `Wet maken loopt - ${name}`;
  }
  return `Verrijking loopt - ${job.law_id}`;
}

// "Beoordelen" navigeert naar het artikel in de editor (met de taak-id als
// query). Taken zonder traject_ref/law_id in de payload tonen een disabled
// knop (zie :disabled hieronder) in plaats van te crashen op een onvolledige
// route.
function review(task) {
  const target = reviewTarget(task);
  if (!target) return;
  router.push(target);
}
</script>

<template>
  <template v-if="running.length > 0">
    <nldd-rich-text><h3>Bezig</h3></nldd-rich-text>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-list variant="simple">
      <nldd-list-item v-for="job in running" :key="job.job_id" size="md">
        <nldd-icon-cell slot="start" size="20">
          <nldd-activity-indicator
            size="16"
            :text="runningText(job)"
          ></nldd-activity-indicator>
        </nldd-icon-cell>
        <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="runningText(job)"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
    <nldd-spacer size="16"></nldd-spacer>
  </template>

  <nldd-inline-dialog
    v-if="tasks.length === 0 && running.length === 0"
    text="Geen open taken."
  ></nldd-inline-dialog>

  <nldd-list v-else-if="tasks.length > 0" variant="simple">
    <nldd-list-item v-for="task in tasks" :key="task.id" size="md">
      <nldd-icon-cell
        slot="start"
        size="20"
        :icon="taskIcon(task)"
        :color="task.task_type === 'job_failed' ? 'critical' : undefined"
      ></nldd-icon-cell>
      <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
      <nldd-text-cell
        :text="task.title"
        :color="task.task_type === 'job_failed' ? 'critical' : undefined"
        :supporting-text="task.task_type === 'job_failed' ? task.payload?.error : undefined"
      ></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-cell>
        <nldd-button
          v-if="task.task_type === 'job_failed'"
          size="sm"
          text="Gezien"
          @click="dismiss(task)"
        ></nldd-button>
        <nldd-button
          v-else
          variant="primary"
          size="sm"
          text="Beoordelen"
          :disabled="!reviewTarget(task) || undefined"
          @click="review(task)"
        ></nldd-button>
      </nldd-cell>
    </nldd-list-item>
  </nldd-list>
</template>
