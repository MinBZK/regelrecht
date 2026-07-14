<script setup>
/**
 * TasksSheet - de takenlijst (job_review + job_failed) van de ingelogde
 * gebruiker. Zelfde sheet-opzet als AboutSheet.vue/SupportSheet.vue: een
 * `nldd-sheet` geteleporteerd naar body, imperatief show()/hide() op
 * `useTasksSheet().isOpen`.
 *
 * Gemount door AppShell, alleen wanneer de gebruiker ingelogd is (de
 * taken-UI is GA, zie TasksButton.vue) - dat is ook de enige plek die
 * `useTasks()` aanroept, dus de 30s-poll start pas dan.
 */
import { nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useTasks } from '../composables/useTasks.js';
import { useTasksSheet } from '../composables/useTasksSheet.js';
import { reviewTarget } from '../lib/taskReview.js';

const router = useRouter();
const { tasks, running, resolveTask } = useTasks();
const { isOpen, close } = useTasksSheet();

const sheetEl = ref(null);

watch(isOpen, async (open) => {
  await nextTick();
  if (open) sheetEl.value?.show();
  else sheetEl.value?.hide();
});

function dismiss(task) {
  resolveTask(task.id, 'dismissed');
}

// "Beoordelen" navigeert naar het artikel in de editor (met de taak-id als
// query) en sluit de sheet. Taken zonder traject_ref/law_id in de payload
// tonen een disabled knop (zie :disabled hieronder) in plaats van te
// crashen op een onvolledige route.
function review(task) {
  const target = reviewTarget(task);
  if (!target) return;
  close();
  router.push(target);
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="right" width="520px" full-height @close="close">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Taken"
          dismiss-text="Sluit"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <template v-if="running.length > 0">
            <nldd-rich-text><h3>Bezig</h3></nldd-rich-text>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list variant="simple">
              <nldd-list-item v-for="job in running" :key="job.job_id" size="md">
                <nldd-icon-cell slot="start" size="20">
                  <nldd-activity-indicator
                    size="16"
                    :text="`Verrijking loopt - ${job.law_id}`"
                  ></nldd-activity-indicator>
                </nldd-icon-cell>
                <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="`Verrijking loopt - ${job.law_id}`"></nldd-text-cell>
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
                :icon="task.task_type === 'job_failed' ? 'alert' : 'tasks'"
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
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
