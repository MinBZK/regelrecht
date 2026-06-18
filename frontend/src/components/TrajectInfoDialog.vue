<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import {
  useTrajectDetail,
  writableSource,
  branchTreeUrl,
} from '../composables/useTrajectDetail.js';
import { deleteTraject, leaveTraject } from '../composables/useTrajects.js';

const props = defineProps({
  /** Whether the sheet is currently open. */
  modelValue: { type: Boolean, default: false },
  /** Traject to show (UUID id, same value the members dialog takes). */
  trajectId: { type: String, default: null },
  /** Traject display name, for the delete confirmation. */
  trajectName: { type: String, default: '' },
});

const emit = defineEmits(['update:modelValue', 'deleted', 'left']);

const sheetEl = ref(null);
const { detail, loading, error: loadError, load } = useTrajectDetail();

// Repo/branch come from the writable-own source; null-safe so an unexpected
// shape renders "onbekend" instead of crashing.
const source = computed(() => writableSource(detail.value));
const repoLabel = computed(() => {
  const s = source.value;
  if (!s || !s.gh_owner || !s.gh_repo) return null;
  return `${s.gh_owner}/${s.gh_repo}`;
});
const repoUrl = computed(() => branchTreeUrl(source.value));
const subpath = computed(() => {
  // No writable source at all → "onbekend", matching the Branch / Base branch
  // fields. Only when a source exists but its path is blank do we show
  // "repo-root" (the meaningful "everything under repo root" default).
  if (!source.value) return 'onbekend';
  const p = source.value.gh_path;
  return p && p.trim() ? p : 'repo-root';
});

// dash for empty optional text fields.
function orDash(v) {
  return v && String(v).trim() ? v : '—';
}

watch(
  () => props.modelValue,
  async (v) => {
    if (v) {
      // Fresh delete-state per open; a stale error from a previous
      // attempt must not greet the user on reopen.
      confirmingDelete.value = false;
      deleteError.value = null;
      confirmingLeave.value = false;
      leaveError.value = null;
      // Kick off the fetch before awaiting nextTick so the request settles
      // promptly (the test drives this with a couple of nextTick flushes).
      const loaded = props.trajectId ? load(props.trajectId) : Promise.resolve();
      await nextTick();
      sheetEl.value?.show();
      await loaded;
    } else {
      await nextTick();
      sheetEl.value?.hide();
    }
  },
);

function close() {
  emit('update:modelValue', false);
}

// Exposed so the unit test can invoke close() directly; in the app the
// dialog is v-model driven and closes via the template handlers.
defineExpose({ close });

// --- Verwijderen (owner-only) ---
const deleteModalEl = ref(null);
const confirmingDelete = ref(false);
const deleteBusy = ref(false);
const deleteError = ref(null);

// Same imperative modal show/hide mirroring as TrajectDocuments' delete
// confirm.
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
  if (!confirmingDelete.value || deleteBusy.value) return; // idempotent: @close + button
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
    close();
  } catch (e) {
    // Keep the modal open so the user can read the error and retry in place.
    deleteError.value = e.message || 'Verwijderen mislukt';
  } finally {
    deleteBusy.value = false;
  }
}

// --- Verlaten (bijdrager) — zelfde bevestigingsmodal als verwijderen. ---
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
  if (!confirmingLeave.value || leaveBusy.value) return; // idempotent: @close + button
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
    close();
  } catch (e) {
    // Keep the modal open so the user can read the error and retry in place.
    leaveError.value = e.message || 'Verlaten mislukt';
  } finally {
    leaveBusy.value = false;
  }
}
</script>

<template>
  <!-- Teleport the sheet out of the toolbar so it doesn't inherit the
       toolbar's positioning / clipping. Matches TrajectMembersDialog. -->
  <Teleport to="body">
    <nldd-sheet
      ref="sheetEl"
      placement="right"
      width="520px"
      full-height
      @close="close"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          :text="trajectName ? `Traject details · ${trajectName}` : 'Traject details'"
          dismiss-text="Sluit"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section v-if="loading">
          <nldd-activity-indicator text="Traject laden" show-text></nldd-activity-indicator>
        </nldd-simple-section>

        <nldd-simple-section v-else-if="loadError">
          <nldd-inline-dialog
            variant="alert"
            :text="loadError.message || 'Fout bij laden'"
          ></nldd-inline-dialog>
        </nldd-simple-section>

        <nldd-simple-section v-else-if="detail">
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
              <nldd-text-cell
                text="Repo"
                max-width="180px"
                vertical-alignment="top"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell
                v-if="repoUrl"
                supporting-text="Opent de traject-branch op GitHub in een nieuw tabblad."
                vertical-alignment="top"
              >
                <!-- nldd-link is the design-system link component. It
                     auto-sets rel='noopener noreferrer' for target='_blank',
                     but we also pass rel explicitly so it is present even
                     before the Lit component upgrades (and is unit-testable).
                     end-icon hints the link leaves the app. -->
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

          <!-- Owner-only: hard delete achter een bevestigingsmodal. -->
          <template v-if="detail.role === 'owner'">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-button
              variant="destructive"
              size="md"
              width="full"
              text="Traject verwijderen"
              @click="askDelete"
            ></nldd-button>
          </template>

          <!-- Bijdrager: traject verlaten, zelfde bevestigingspatroon als delete. -->
          <template v-else-if="detail.role">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-button
              variant="destructive"
              size="md"
              width="full"
              text="Traject verlaten"
              @click="askLeave"
            ></nldd-button>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>

  <!-- Delete confirmation — NLDD modal, consistent with TrajectDocuments'
       delete dialog. -->
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

  <!-- Leave confirmation — same NLDD modal pattern as delete, from the
       contributor's side. -->
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
