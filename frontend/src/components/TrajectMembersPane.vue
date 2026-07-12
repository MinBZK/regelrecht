<script setup>
// Traject members as a Home main-pane view (was TrajectMembersDialog's content).
// The member list + pending invites live in the pane; "Lid uitnodigen" opens the
// invite form in its own sheet. Loads on mount and whenever the traject changes.
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useTrajectMembers } from '../composables/useTrajectMembers.js';

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
  invite,
  updateRole,
  removeMember,
  removeInvite,
} = useTrajectMembers();

function reload() {
  if (props.trajectId) load(props.trajectId);
}
onMounted(reload);
watch(() => props.trajectId, reload);

const isOwner = computed(() => callerRole.value === 'owner');
function roleLabel(role) {
  return role === 'owner' ? 'Eigenaar' : 'Bijdrager';
}

const ownerCount = computed(() => members.value.filter((m) => m.role === 'owner').length);
function canManageMember(m) {
  return isOwner.value && (m.role !== 'owner' || ownerCount.value > 1);
}

function onRoleChange(e) {
  if (e.detail?.selected) inviteRole.value = e.detail.value;
}

// Invite form (in its own sheet)
const inviteSheetEl = ref(null);
const inviteEmail = ref('');
const inviteRole = ref('contributor');
const inviteBusy = ref(false);
const inviteError = ref(null);
const inviteResult = ref(null);

function openInvite() {
  inviteEmail.value = '';
  inviteRole.value = 'contributor';
  inviteError.value = null;
  inviteResult.value = null;
  nextTick(() => inviteSheetEl.value?.show?.());
}
function closeInvite() {
  inviteSheetEl.value?.hide?.();
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

async function submitInvite() {
  inviteError.value = null;
  inviteResult.value = null;
  if (!inviteEmail.value.trim()) {
    inviteError.value = 'E-mailadres is verplicht';
    return;
  }
  inviteBusy.value = true;
  try {
    const body = await invite(props.trajectId, inviteEmail.value.trim(), inviteRole.value);
    inviteEmail.value = '';
    inviteResult.value =
      body.status === 'pending'
        ? `Uitnodiging klaargezet voor ${body.email}. Toegang wordt actief bij de eerste login.`
        : `${body.email} is toegevoegd.`;
  } catch (e) {
    inviteError.value = e.message || 'Uitnodigen mislukt';
  } finally {
    inviteBusy.value = false;
  }
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
</script>

<template>
  <nldd-simple-section width="full">
    <nldd-title id="instellingen-titel" size="3"><h3>Leden</h3></nldd-title>
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-toolbar v-if="isOwner" label="Ledenacties">
      <nldd-toolbar-item slot="start">
        <nldd-icon-button icon="plus-small" text="Lid uitnodigen" @click="openInvite"></nldd-icon-button>
      </nldd-toolbar-item>
    </nldd-toolbar>
    <nldd-spacer v-if="isOwner" size="16"></nldd-spacer>

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
                <nldd-menu-item text="Maak eigenaar" @select="changeMemberRole(m, 'owner')"></nldd-menu-item>
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
    </nldd-list>

    <template v-if="pendingInvites.length > 0">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="4"><h2>Openstaande uitnodigingen</h2></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="box">
        <template v-for="inv in pendingInvites" :key="inv.email">
          <nldd-list-item size="md">
            <nldd-spacer-cell slot="start" size="12"></nldd-spacer-cell>
            <nldd-icon-cell slot="start" size="20"><nldd-icon name="user"></nldd-icon></nldd-icon-cell>
            <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
            <nldd-text-cell :text="inv.email" supporting-text="Wacht op eerste login"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell>
              <span class="members-pending-role">{{ roleLabel(inv.role) }}</span>
            </nldd-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell v-if="isOwner">
              <nldd-button
                variant="ghost"
                size="sm"
                text="Trek in"
                :disabled="rowBusy.has(inv.email) || undefined"
                @click="clickRemoveInvite(inv)"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
          <div v-if="rowError.get(inv.email)" class="members-row-error">
            {{ rowError.get(inv.email) }}
          </div>
        </template>
      </nldd-list>
    </template>
    </template>
  </nldd-simple-section>

  <!-- Invite form in its own sheet. -->
  <Teleport to="body">
    <nldd-sheet ref="inviteSheetEl" placement="right" width="480px" full-height @close="closeInvite">
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar slot="header" text="Lid uitnodigen" dismiss-text="Annuleer" @dismiss="closeInvite"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-form>
            <form novalidate @submit.prevent="submitInvite">
              <nldd-form-field label="E-mail">
                <nldd-text-field
                  size="md"
                  type="email"
                  name="email"
                  :value="inviteEmail"
                  :invalid="inviteError ? true : undefined"
                  :error-message="inviteError ? 'invite-email-error' : undefined"
                  @input="inviteEmail = $event.target?.value ?? $event.detail?.value ?? inviteEmail"
                ></nldd-text-field>
                <nldd-form-field-help-text>
                  Toegang wordt actief bij de eerste login als er nog geen account bestaat.
                </nldd-form-field-help-text>
                <nldd-form-field-error-text id="invite-email-error">
                  {{ inviteError }}
                </nldd-form-field-error-text>
              </nldd-form-field>

              <nldd-form-field label="Rol">
                <nldd-toggle-button-group type="radio" size="md" accessible-label="Rol" @change="onRoleChange">
                  <nldd-toggle-button value="contributor" text="Bijdrager" :selected="inviteRole === 'contributor' || undefined"></nldd-toggle-button>
                  <nldd-toggle-button value="owner" text="Eigenaar" :selected="inviteRole === 'owner' || undefined"></nldd-toggle-button>
                </nldd-toggle-button-group>
              </nldd-form-field>

              <div v-if="inviteResult" class="members-info">{{ inviteResult }}</div>

              <nldd-form-actions>
                <nldd-button
                  variant="primary"
                  size="md"
                  type="submit"
                  width="full"
                  :text="inviteBusy ? 'Bezig…' : 'Nodig lid uit'"
                  :disabled="inviteBusy || undefined"
                ></nldd-button>
              </nldd-form-actions>
            </form>
          </nldd-form>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
.member-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.members-info {
  font-size: 13px;
  color: var(--semantics-content-secondary-color, #555);
  margin: 12px 0 4px;
}
.members-row-error {
  color: var(--nldd-color-text-error, #c62828);
  font-size: 13px;
  margin-top: 8px;
}
.members-pending-role {
  font-size: 13px;
  color: var(--semantics-content-secondary-color, #555);
  text-transform: capitalize;
}
</style>
