<script setup>
/**
 * TasksSheet - de takenlijst (job_review + job_failed) van de ingelogde
 * gebruiker. Zelfde sheet-opzet als AboutSheet.vue/SupportSheet.vue: een
 * `nldd-sheet` geteleporteerd naar body, imperatief show()/hide() op
 * `useTasksSheet().isOpen`.
 *
 * Gemount door AppShell, alleen wanneer de `tasks.job_review`-flag aan
 * staat EN de gebruiker ingelogd is (zie TasksButton.vue) - dat is ook de
 * enige plek die `useTasks()` aanroept, dus de 30s-poll start pas dan.
 */
import { nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useTasks } from '../composables/useTasks.js';
import { useTasksSheet } from '../composables/useTasksSheet.js';
import { reviewTarget } from '../lib/taskReview.js';

const router = useRouter();
const { tasks, resolveTask } = useTasks();
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
          <nldd-inline-dialog
            v-if="tasks.length === 0"
            text="Geen open taken."
          ></nldd-inline-dialog>

          <div v-else class="tasks-list">
            <nldd-inline-dialog
              v-for="task in tasks"
              :key="task.id"
              :variant="task.task_type === 'job_failed' ? 'alert' : undefined"
              :text="task.title"
              :supporting-text="
                task.task_type === 'job_failed'
                  ? task.payload?.error
                  : 'Verrijking is klaar om te beoordelen.'
              "
            >
              <nldd-button
                v-if="task.task_type === 'job_failed'"
                slot="actions"
                size="md"
                text="Gezien"
                @click="dismiss(task)"
              ></nldd-button>
              <nldd-button
                v-else
                slot="actions"
                variant="primary"
                size="md"
                text="Beoordelen"
                :disabled="!reviewTarget(task) || undefined"
                @click="review(task)"
              ></nldd-button>
            </nldd-inline-dialog>
          </div>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
/* Stapel de takenkaarten met een kleine tussenruimte - zelfde patroon als
   ConversionStatus.vue (daar ook als custom CSS gerapporteerd). */
.tasks-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
