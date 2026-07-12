<script setup>
// Traject-details as a Home main-pane view (was TrajectInfoDialog's content).
// Renders the detail list + owner-delete / contributor-leave, each behind a
// confirmation modal. Loads on mount and whenever the traject changes.
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import {
  useTrajectDetail,
  writableSource,
  branchTreeUrl,
} from '../composables/useTrajectDetail.js';
import { deleteTraject, leaveTraject } from '../composables/useTrajects.js';

const props = defineProps({
  /** Traject to show (UUID id). */
  trajectId: { type: String, default: null },
  /** Traject display name, for the delete/leave confirmation. */
  trajectName: { type: String, default: '' },
});

const emit = defineEmits(['deleted', 'left']);

const { detail, loading, error: loadError, load } = useTrajectDetail();

function reload() {
  if (props.trajectId) load(props.trajectId);
}
onMounted(reload);
watch(() => props.trajectId, reload);

const source = computed(() => writableSource(detail.value));
const repoLabel = computed(() => {
  const s = source.value;
  if (!s || !s.gh_owner || !s.gh_repo) return null;
  return `${s.gh_owner}/${s.gh_repo}`;
});
const repoUrl = computed(() => branchTreeUrl(source.value));
const subpath = computed(() => {
  if (!source.value) return 'onbekend';
  const p = source.value.gh_path;
  return p && p.trim() ? p : 'repo-root';
});

function orDash(v) {
  return v && String(v).trim() ? v : '—';
}

// --- Verwijderen (owner-only) ---
const deleteModalEl = ref(null);
const confirmingDelete = ref(false);
const deleteBusy = ref(false);
const deleteError = ref(null);
watch(confirmingDelete, async (open) => {
  await nextTick();
  const el = deleteModalEl.value;
  if (!el) return;
  if (open) el.show?.();
  else el.hide?.();
});
function askDelete() {
  deleteError.value = null;
  confirmingDelete.value = true;
}
function cancelDelete() {
  if (!confirmingDelete.value || deleteBusy.value) return;
  confirmingDelete.value = false;
}
async function confirmDelete() {
  if (deleteBusy.value) return;
  deleteBusy.value = true;
  deleteError.value = null;
  try {
    await deleteTraject(props.trajectId);
    confirmingDelete.value = false;
    emit('deleted', props.trajectId);
  } catch (e) {
    deleteError.value = e.message || 'Verwijderen mislukt';
  } finally {
    deleteBusy.value = false;
  }
}

// --- Verlaten (bijdrager) ---
const leaveModalEl = ref(null);
const confirmingLeave = ref(false);
const leaveBusy = ref(false);
const leaveError = ref(null);
watch(confirmingLeave, async (open) => {
  await nextTick();
  const el = leaveModalEl.value;
  if (!el) return;
  if (open) el.show?.();
  else el.hide?.();
});
function askLeave() {
  leaveError.value = null;
  confirmingLeave.value = true;
}
function cancelLeave() {
  if (!confirmingLeave.value || leaveBusy.value) return;
  confirmingLeave.value = false;
}
async function confirmLeave() {
  if (leaveBusy.value) return;
  leaveBusy.value = true;
  leaveError.value = null;
  try {
    await leaveTraject(props.trajectId);
    confirmingLeave.value = false;
    emit('left', props.trajectId);
  } catch (e) {
    leaveError.value = e.message || 'Verlaten mislukt';
  } finally {
    leaveBusy.value = false;
  }
}
</script>

<template>
  <nldd-simple-section width="full">
    <nldd-title id="instellingen-titel" size="3"><h3>Traject details</h3></nldd-title>
    <nldd-spacer size="16"></nldd-spacer>

    <nldd-activity-indicator v-if="loading" text="Traject laden" show-text></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="loadError" variant="alert" :text="loadError.message || 'Fout bij laden'"></nldd-inline-dialog>
    <template v-else-if="detail">
    <nldd-list variant="box">
      <nldd-list-item size="md">
        <nldd-text-cell text="Naam" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="detail.name"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Beschrijving" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="orDash(detail.description)"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Status" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="detail.status"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Jouw rol" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="detail.role"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Repo" max-width="180px" vertical-alignment="top"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell
          v-if="repoUrl"
          supporting-text="Opent de traject-branch op GitHub in een nieuw tabblad."
          vertical-alignment="top"
        >
          <nldd-link
            size="md"
            :href="repoUrl"
            target="_blank"
            rel="noopener noreferrer"
            end-icon="external-link"
            :text="repoLabel"
          ></nldd-link>
        </nldd-text-cell>
        <nldd-text-cell v-else :text="repoLabel || 'onbekend'"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Branch" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="source?.gh_branch || 'onbekend'"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Base branch" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="source?.gh_base_branch || 'onbekend'"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Subpath" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="subpath"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>

    <template v-if="detail.role === 'owner'">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-button variant="destructive" size="md" text="Traject verwijderen" @click="askDelete"></nldd-button>
    </template>
    <template v-else-if="detail.role">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-button variant="destructive" size="md" text="Traject verlaten" @click="askLeave"></nldd-button>
    </template>
    </template>
  </nldd-simple-section>

  <Teleport to="body">
    <nldd-modal-dialog
      ref="deleteModalEl"
      variant="alert"
      :text="`Traject ${trajectName} verwijderen?`"
      supporting-text="Het traject wordt definitief verwijderd, inclusief leden en uitnodigingen. De traject-branch op GitHub blijft bestaan. Dit kan niet ongedaan worden gemaakt."
      @close="cancelDelete"
    >
      <nldd-inline-dialog v-if="deleteError" variant="alert" :text="deleteError"></nldd-inline-dialog>
      <nldd-button slot="actions" variant="primary" text="Behoud traject" @click="cancelDelete"></nldd-button>
      <nldd-button
        slot="actions"
        variant="destructive"
        :text="deleteBusy ? 'Bezig…' : 'Verwijder traject'"
        :disabled="deleteBusy || undefined"
        @click="confirmDelete"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>

  <Teleport to="body">
    <nldd-modal-dialog
      ref="leaveModalEl"
      variant="alert"
      :text="`Traject ${trajectName} verlaten?`"
      supporting-text="Je verlaat dit traject definitief en verliest meteen je toegang. Wil je later weer bijdragen, dan moet een eigenaar je opnieuw uitnodigen."
      @close="cancelLeave"
    >
      <nldd-inline-dialog v-if="leaveError" variant="alert" :text="leaveError"></nldd-inline-dialog>
      <nldd-button slot="actions" variant="primary" text="Blijf in traject" @click="cancelLeave"></nldd-button>
      <nldd-button
        slot="actions"
        variant="destructive"
        :text="leaveBusy ? 'Bezig…' : 'Verlaat traject'"
        :disabled="leaveBusy || undefined"
        @click="confirmLeave"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>
