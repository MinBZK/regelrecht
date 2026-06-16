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
          :text="`Leden — ${trajectName}`"
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
          <nldd-simple-section heading="Actieve leden">
            <nldd-list variant="box">
              <template v-for="m in members" :key="m.account_id">
                <nldd-list-item size="md">
                  <nldd-text-cell
                    :text="m.name || m.email"
                    :supporting-text="m.name ? m.email : null"
                  ></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-cell>
                    <nldd-dropdown
                      size="md"
                      @change="changeMemberRole(m, $event.detail?.value ?? $event.target?.value ?? m.role)"
                    >
                      <select
                        :value="m.role"
                        :disabled="!isOwner || rowBusy.has(m.account_id) || undefined"
                      >
                        <option value="owner">Owner</option>
                        <option value="contributor">Contributor</option>
                      </select>
                    </nldd-dropdown>
                  </nldd-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-cell v-if="isOwner">
                    <nldd-button
                      variant="ghost"
                      size="sm"
                      text="Verwijder"
                      :disabled="rowBusy.has(m.account_id) || undefined"
                      @click="clickRemoveMember(m)"
                    ></nldd-button>
                  </nldd-cell>
                </nldd-list-item>
                <div v-if="rowError.get(m.account_id)" class="members-row-error">
                  {{ rowError.get(m.account_id) }}
                </div>
              </template>
            </nldd-list>
          </nldd-simple-section>

          <nldd-simple-section
            v-if="pendingInvites.length > 0"
            heading="Openstaande uitnodigingen"
          >
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
          </nldd-simple-section>

          <nldd-simple-section v-if="isOwner" heading="Iemand uitnodigen">
            <nldd-form-field label="E-mail" supporting-label="Toegang wordt actief bij de eerste login als er nog geen account bestaat.">
              <nldd-text-field
                size="md"
                type="email"
                :value="inviteEmail"
                :invalid="inviteError ? true : undefined"
                :error-message="inviteError ? 'invite-email-error' : undefined"
                @input="inviteEmail = $event.target?.value ?? $event.detail?.value ?? inviteEmail"
              ></nldd-text-field>
              <nldd-form-field-error-text id="invite-email-error">
                {{ inviteError }}
              </nldd-form-field-error-text>
            </nldd-form-field>

            <nldd-form-field label="Rol">
              <nldd-dropdown
                size="md"
                @change="inviteRole = $event.detail?.value ?? $event.target?.value ?? inviteRole"
              >
                <select :value="inviteRole">
                  <option value="contributor">Contributor</option>
                  <option value="owner">Owner</option>
                </select>
              </nldd-dropdown>
            </nldd-form-field>

            <div v-if="inviteResult" class="members-info">{{ inviteResult }}</div>
            <nldd-button
              variant="primary"
              size="md"
              text="Uitnodigen"
              :loading="inviteBusy || undefined"
              @click="submitInvite"
            ></nldd-button>
          </nldd-simple-section>

          <nldd-simple-section v-if="!isOwner && callerRole" heading="Dit traject verlaten">
            <p class="members-leave-hint">
              Je verliest direct toegang tot dit traject. Een owner kan je later
              opnieuw uitnodigen.
            </p>
            <div v-if="leaveError" class="members-error">{{ leaveError }}</div>
            <nldd-button
              variant="ghost"
              size="md"
              text="Verlaat traject"
              :loading="leaveBusy || undefined"
              @click="clickLeave"
            ></nldd-button>
          </nldd-simple-section>
        </template>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
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
