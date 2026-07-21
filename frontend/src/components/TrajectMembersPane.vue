<script setup>
// Traject members as a Home main-pane view. The member list + pending invites
// live here; "Uitnodigen" opens the shared InviteMembersSheet (via useAddActions),
// which also opens from the universal "+" in the header. Loads on mount, on
// traject change, and whenever an invite elsewhere signals a change.
import { computed, onMounted, ref, watch } from 'vue';
import { useTrajectMembers } from '../composables/useTrajectMembers.js';
import { useAddActions } from '../composables/useAddActions.js';
import { paneChromeVisible } from '../constants.js';

const props = defineProps({
  /** Traject to manage (UUID id). */
  trajectId: { type: String, default: null },
});

const {
  members,
  pendingInvites,
  callerRole,
  loading,
  error: loadError,
  load,
  updateRole,
  removeMember,
  removeInvite,
} = useTrajectMembers();

const { triggerInviteMembers, membersChanged } = useAddActions();

function reload() {
  if (props.trajectId) load(props.trajectId);
}
onMounted(reload);
watch(() => props.trajectId, reload);
// De InviteMembersSheet (een andere useTrajectMembers-instance) bumpt dit na een
// geslaagde invite; herladen zodat de nieuwe openstaande uitnodiging verschijnt.
watch(membersChanged, reload);

const isOwner = computed(() => callerRole.value === 'owner');
function roleLabel(role) {
  return role === 'owner' ? 'Beheerder' : 'Bijdrager';
}

const ownerCount = computed(() => members.value.filter((m) => m.role === 'owner').length);
function canManageMember(m) {
  return isOwner.value && (m.role !== 'owner' || ownerCount.value > 1);
}

// Row-level busy + error indicators keyed by member account_id / invite email.
const rowBusy = ref(new Set());
const rowError = ref(new Map());
function markBusy(key, busy) {
  const next = new Set(rowBusy.value);
  if (busy) next.add(key);
  else next.delete(key);
  rowBusy.value = next;
}
function setRowError(key, message) {
  const next = new Map(rowError.value);
  if (message) next.set(key, message);
  else next.delete(key);
  rowError.value = next;
}

async function changeMemberRole(member, newRole) {
  if (member.role === newRole) return;
  setRowError(member.account_id, null);
  markBusy(member.account_id, true);
  try {
    await updateRole(props.trajectId, member.account_id, newRole);
  } catch (e) {
    setRowError(member.account_id, e.message || 'Wijzigen mislukt');
  } finally {
    markBusy(member.account_id, false);
  }
}

async function clickRemoveMember(member) {
  setRowError(member.account_id, null);
  markBusy(member.account_id, true);
  try {
    await removeMember(props.trajectId, member.account_id);
  } catch (e) {
    setRowError(member.account_id, e.message || 'Verwijderen mislukt');
  } finally {
    markBusy(member.account_id, false);
  }
}

async function clickRemoveInvite(inv) {
  setRowError(inv.email, null);
  markBusy(inv.email, true);
  try {
    await removeInvite(props.trajectId, inv.email);
  } catch (e) {
    setRowError(inv.email, e.message || 'Intrekken mislukt');
  } finally {
    markBusy(inv.email, false);
  }
}

// Retracting an invite is destructive, so confirm it in a modal that names the
// address before the actual removeInvite runs.
const confirmDialogEl = ref(null);
const confirmInvite = ref(null);

function askRemoveInvite(inv) {
  confirmInvite.value = inv;
  confirmDialogEl.value?.show?.();
}
function cancelRemoveInvite() {
  confirmDialogEl.value?.hide?.();
}
async function confirmRemoveInvite() {
  const inv = confirmInvite.value;
  confirmDialogEl.value?.hide?.();
  if (inv) await clickRemoveInvite(inv);
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title v-if="paneChromeVisible(loading)" id="instellingen-pane-titel" size="3"><h3>Leden</h3></nldd-title>
    <nldd-spacer v-if="paneChromeVisible(loading)" size="16"></nldd-spacer>
    <nldd-toolbar v-if="isOwner && paneChromeVisible(loading)" label="Ledenacties">
      <nldd-toolbar-item slot="start">
        <nldd-button variant="secondary" size="md" start-icon="plus-small" text="Uitnodigen" @click="triggerInviteMembers"></nldd-button>
      </nldd-toolbar-item>
    </nldd-toolbar>
    <nldd-spacer v-if="isOwner && paneChromeVisible(loading)" size="16"></nldd-spacer>

    <nldd-activity-indicator v-if="loading" text="Leden laden" show-text></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="loadError" variant="alert" text="Leden niet geladen" :supporting-text="loadError.message"></nldd-inline-dialog>
    <template v-else>
    <nldd-list variant="box">
      <template v-for="m in members" :key="m.account_id">
        <nldd-list-item size="md">
          <nldd-spacer-cell slot="start" size="12"></nldd-spacer-cell>
          <nldd-icon-cell slot="start" size="20"><nldd-icon name="user"></nldd-icon></nldd-icon-cell>
          <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
          <nldd-text-cell :supporting-text="m.name ? m.email : null">
            <span class="member-name">
              <span>{{ m.name || m.email }}</span>
              <nldd-tag size="sm" :color="m.role === 'owner' ? 'accent' : 'neutral'" :text="roleLabel(m.role)"></nldd-tag>
            </span>
          </nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell v-if="canManageMember(m)">
            <nldd-icon-button
              :id="`member-more-${m.account_id}`"
              icon="more"
              text="Meer acties"
              tooltip-timing="never"
              popup-type="menu"
              :popovertarget="`member-menu-${m.account_id}`"
              :disabled="rowBusy.has(m.account_id) || undefined"
            ></nldd-icon-button>
            <nldd-menu :id="`member-menu-${m.account_id}`" :anchor="`member-more-${m.account_id}`">
              <nldd-menu-item
                v-if="m.role === 'owner'"
                text="Maak bijdrager"
                @select="changeMemberRole(m, 'contributor')"
              ></nldd-menu-item>
              <template v-else>
                <nldd-menu-item text="Maak beheerder" @select="changeMemberRole(m, 'owner')"></nldd-menu-item>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item text="Verwijder lid" destructive @select="clickRemoveMember(m)"></nldd-menu-item>
              </template>
            </nldd-menu>
          </nldd-cell>
        </nldd-list-item>
        <div v-if="rowError.get(m.account_id)" class="members-row-error">
          {{ rowError.get(m.account_id) }}
        </div>
      </template>
      <!-- Pending invites live in the same list so a long roster stays filterable
           instead of scrolling them out of view in a separate section below. -->
      <template v-for="inv in pendingInvites" :key="inv.email">
        <nldd-list-item size="md">
          <nldd-spacer-cell slot="start" size="12"></nldd-spacer-cell>
          <nldd-icon-cell slot="start" size="20"><nldd-icon name="user"></nldd-icon></nldd-icon-cell>
          <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
          <nldd-text-cell supporting-text="Openstaande uitnodiging. Wacht op eerste login voor activatie.">
            <span class="member-name">
              <span>{{ inv.email }}</span>
              <nldd-tag size="sm" :color="inv.role === 'owner' ? 'accent' : 'neutral'" :text="roleLabel(inv.role)"></nldd-tag>
            </span>
          </nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell v-if="isOwner">
            <nldd-button
              variant="destructive"
              size="sm"
              text="Intrekken"
              :disabled="rowBusy.has(inv.email) || undefined"
              @click="askRemoveInvite(inv)"
            ></nldd-button>
          </nldd-cell>
        </nldd-list-item>
        <div v-if="rowError.get(inv.email)" class="members-row-error">
          {{ rowError.get(inv.email) }}
        </div>
      </template>
    </nldd-list>
    </template>
  </nldd-simple-section>

  <!-- Confirm dialog for retracting a pending invite. -->
  <Teleport to="body">
    <nldd-modal-dialog
      ref="confirmDialogEl"
      :text="confirmInvite ? `Uitnodiging voor ${confirmInvite.email} intrekken?` : ''"
      supporting-text="Je kunt deze persoon op een later moment weer uitnodigen."
    >
      <nldd-button slot="actions" variant="secondary" size="md" width="full" text="Behoud uitnodiging" @click="cancelRemoveInvite"></nldd-button>
      <nldd-button slot="actions" variant="destructive" size="md" width="full" text="Trek uitnodiging in" @click="confirmRemoveInvite"></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>

<style scoped>
.member-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.members-row-error {
  color: var(--nldd-color-text-error, #c62828);
  font-size: 13px;
  margin-top: 8px;
}
</style>
