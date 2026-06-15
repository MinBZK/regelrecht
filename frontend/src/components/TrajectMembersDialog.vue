<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useTrajectMembers } from '../composables/useTrajectMembers.js';
import { refreshTrajects } from '../composables/useTrajects.js';

const props = defineProps({
  /** Whether the sheet is currently open. */
  modelValue: { type: Boolean, default: false },
  /** Traject to manage. */
  trajectId: { type: String, default: null },
  /** Traject display name, for the sheet header. */
  trajectName: { type: String, default: '' },
});

const emit = defineEmits(['update:modelValue']);

const sheetEl = ref(null);

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
  leaveTraject,
} = useTrajectMembers();

const isOwner = computed(() => callerRole.value === 'owner');

function roleLabel(role) {
  return role === 'owner' ? 'Eigenaar' : 'Bijdrager';
}

// A traject must always keep at least one owner, so the sole owner can be
// neither demoted nor removed — they get no actions menu at all. Promote
// someone else first (or delete the traject).
const ownerCount = computed(() => members.value.filter((m) => m.role === 'owner').length);
function canManageMember(m) {
  return isOwner.value && (m.role !== 'owner' || ownerCount.value > 1);
}

// The radio toggle-button-group fires `change` from each affected button; only
// the newly-selected one carries selected:true.
function onRoleChange(e) {
  if (e.detail?.selected) inviteRole.value = e.detail.value;
}

// Invite form
const inviteEmail = ref('');
const inviteRole = ref('contributor');
const inviteBusy = ref(false);
const inviteError = ref(null);
const inviteResult = ref(null);

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

watch(
  () => props.modelValue,
  async (v) => {
    await nextTick();
    if (v) {
      inviteEmail.value = '';
      inviteRole.value = 'contributor';
      inviteError.value = null;
      inviteResult.value = null;
      rowBusy.value = new Set();
      rowError.value = new Map();
      if (props.trajectId) {
        await load(props.trajectId);
      }
      sheetEl.value?.show();
    } else {
      sheetEl.value?.hide();
    }
  },
);

function close() {
  emit('update:modelValue', false);
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

const leaveBusy = ref(false);
const leaveError = ref(null);

async function clickLeave() {
  leaveError.value = null;
  leaveBusy.value = true;
  try {
    await leaveTraject(props.trajectId);
    await refreshTrajects();
    close();
  } catch (e) {
    leaveError.value = e.message || 'Verlaten mislukt';
  } finally {
    leaveBusy.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet
      ref="sheetEl"
      placement="right"
      width="520px"
      full-height
      @close="close"
    >
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar
          slot="header"
          :text="trajectName ? `Leden · ${trajectName}` : 'Leden'"
          dismiss-text="Sluit"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section v-if="loading">
          <p class="members-status">Laden…</p>
        </nldd-simple-section>

        <nldd-simple-section v-else-if="loadError">
          <p class="members-error">{{ loadError.message || 'Fout bij laden' }}</p>
        </nldd-simple-section>

        <template v-else>
          <nldd-simple-section>
            <nldd-list variant="box">
              <template v-for="m in members" :key="m.account_id">
                <nldd-list-item size="md">
                  <nldd-text-cell :supporting-text="m.name ? m.email : null">
                    <span class="member-name">
                      <span>{{ m.name || m.email }}</span>
                      <nldd-tag
                        size="sm"
                        :color="m.role === 'owner' ? 'accent' : 'neutral'"
                        :text="roleLabel(m.role)"
                      ></nldd-tag>
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
                      <!-- Een eigenaar kun je niet rechtstreeks verwijderen (een
                           traject houdt altijd minstens één eigenaar). Degradeer
                           'm eerst naar bijdrager — dat kan pas als er nóg een
                           eigenaar is — daarna verschijnt 'Verwijder lid'. -->
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
              <nldd-spacer size="32"></nldd-spacer>
              <nldd-title size="4"><h2>Openstaande uitnodigingen</h2></nldd-title>
              <nldd-spacer size="12"></nldd-spacer>
              <nldd-list variant="box">
                <template v-for="inv in pendingInvites" :key="inv.email">
                  <nldd-list-item size="md">
                    <nldd-text-cell
                      :text="inv.email"
                      supporting-text="Wacht op eerste login"
                    ></nldd-text-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-cell>
                      <span class="members-pending-role">{{ inv.role }}</span>
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

            <template v-if="isOwner">
              <nldd-spacer size="32"></nldd-spacer>
              <nldd-title size="4"><h2>Lid uitnodigen</h2></nldd-title>
              <nldd-spacer size="12"></nldd-spacer>
              <!-- nldd-form renders a real light-DOM <form>; provide our own so
                   it skips the MutationObserver child-migration (which would
                   fight Vue's DOM). The inner form owns the submit. -->
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
                    <nldd-toggle-button-group
                      type="radio"
                      size="md"
                      accessible-label="Rol"
                      @change="onRoleChange"
                    >
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
            </template>

            <template v-if="!isOwner && callerRole">
              <nldd-spacer size="32"></nldd-spacer>
              <nldd-title size="4"><h2>Dit traject verlaten</h2></nldd-title>
              <nldd-spacer size="12"></nldd-spacer>
              <p class="members-leave-hint">
                Je verliest direct toegang tot dit traject. Een owner kan je later
                opnieuw uitnodigen.
              </p>
              <div v-if="leaveError" class="members-error">{{ leaveError }}</div>
              <nldd-button
                variant="ghost"
                size="md"
                :text="leaveBusy ? 'Bezig…' : 'Verlaat traject'"
                :disabled="leaveBusy || undefined"
                @click="clickLeave"
              ></nldd-button>
            </template>
          </nldd-simple-section>
        </template>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
/* Name + role tag on one line, vertically centered. */
.member-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.members-status,
.members-leave-hint {
  font-size: 13px;
  color: var(--semantics-content-secondary-color, #555);
  margin: 8px 0;
}
.members-info {
  font-size: 13px;
  color: var(--semantics-content-secondary-color, #555);
  margin: 12px 0 4px;
}
.members-error,
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
