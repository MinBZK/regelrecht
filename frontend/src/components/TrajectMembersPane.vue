<script setup>
// Traject members as a Home main-pane view (was TrajectMembersDialog's content).
// The member list + pending invites live in the pane; "Lid uitnodigen" opens the
// invite form in its own sheet. Loads on mount and whenever the traject changes.
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useTrajectMembers } from '../composables/useTrajectMembers.js';
import { paneChromeVisible } from '../constants.js';

const props = defineProps({
  /** Traject to manage (UUID id). */
  trajectId: { type: String, default: null },
});

const {
  members,
  pendingInvites,
  callerRole,
  trajectName,
  loading,
  error: loadError,
  load,
  inviteMany,
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
  return role === 'owner' ? 'Beheerder' : 'Bijdrager';
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
const inviteInputRef = ref(null);
const inviteEmails = ref([]); // committed tokens
const invitePending = ref(''); // uncommitted input text
const inviteRole = ref('contributor');
const inviteBusy = ref(false);
const inviteError = ref(null);
const inviteResult = ref(null);

// Swap the form for the success view once every address in the batch went
// through. A partial batch keeps the form: the failures stay in the field so
// they can be corrected and retried, with the outcome shown inline.
const inviteSucceeded = computed(
  () =>
    !!inviteResult.value &&
    inviteResult.value.failed.length === 0 &&
    inviteResult.value.succeeded.length > 0,
);

// Reset the form to its empty state and re-seed the (async-mounted) token
// field. Shared by opening the sheet and by "Meer uitnodigen".
async function resetInviteForm() {
  inviteEmails.value = [];
  invitePending.value = '';
  inviteRole.value = 'contributor';
  inviteError.value = null;
  inviteResult.value = null;
  await nextTick();
  if (inviteInputRef.value) inviteInputRef.value.values = [];
}

async function openInvite() {
  await resetInviteForm();
  inviteSheetEl.value?.show?.();
  await nextTick();
  inviteInputRef.value?.focus?.();
}
function closeInvite() {
  inviteSheetEl.value?.hide?.();
}

// "Meer uitnodigen" on the success view: swap it back for a fresh form.
async function inviteAgain() {
  await resetInviteForm();
  await nextTick();
  inviteInputRef.value?.focus?.();
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

// Token-field handlers. Besides tracking the committed tokens / pending text,
// they clear the "voer een adres in" error as soon as the user acts, so the
// error and invalid outline disappear the moment they start correcting it.
function onEmailsChange(e) {
  inviteEmails.value = e.detail?.values ?? inviteEmails.value;
  if (inviteError.value) inviteError.value = null;
}
function onEmailsInput(e) {
  invitePending.value = e.detail?.value ?? invitePending.value;
  if (inviteError.value) inviteError.value = null;
}

async function submitInvite() {
  inviteError.value = null;
  inviteResult.value = null;
  // Include the current (uncommitted) input text alongside the committed tokens,
  // so a last address typed without pressing Enter isn't silently dropped.
  const raw = [...inviteEmails.value];
  const pending = invitePending.value.trim();
  if (pending) raw.push(pending);
  const emails = [...new Set(raw.map((e) => e.trim()).filter(Boolean))];
  if (emails.length === 0) {
    inviteError.value = 'Voer minstens één e-mailadres in';
    // Pull focus back to the field so the just-revealed error is where the
    // user is looking and they can start typing straight away.
    await nextTick();
    inviteInputRef.value?.focus?.();
    return;
  }
  inviteBusy.value = true;
  try {
    const { succeeded, failed } = await inviteMany(props.trajectId, emails, inviteRole.value);
    inviteResult.value = { succeeded, failed };
    // Keep only the failures in the field so the user can fix and retry.
    inviteEmails.value = failed.map((f) => f.email);
    invitePending.value = '';
    if (inviteInputRef.value) inviteInputRef.value.values = inviteEmails.value;
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
        <nldd-button variant="secondary" size="md" start-icon="plus-small" text="Uitnodigen" @click="openInvite"></nldd-button>
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

  <!-- Invite form in its own sheet. -->
  <Teleport to="body">
    <nldd-sheet ref="inviteSheetEl" placement="right" width="480px" full-height @close="closeInvite">
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar slot="header" text="Uitnodigen" dismiss-text="Annuleer" @dismiss="closeInvite"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <!-- Success view: a new view in the sheet, shown once the whole batch was sent. -->
          <nldd-inline-dialog
            v-if="inviteSucceeded"
            variant="success"
            :text="`${inviteResult.succeeded.length} uitnodiging${inviteResult.succeeded.length === 1 ? '' : 'en'} verstuurd`"
            supporting-text="Toegang wordt actief bij de eerste login."
          >
            <nldd-button slot="actions" variant="primary" size="md" width="full" text="Sluit" @click="closeInvite"></nldd-button>
            <nldd-button slot="actions" variant="secondary" size="md" width="full" text="Meer uitnodigen" @click="inviteAgain"></nldd-button>
          </nldd-inline-dialog>

          <nldd-form v-else>
            <form novalidate @submit.prevent="submitInvite">
              <nldd-form-field label="E-mailadressen">
                <nldd-token-field
                  ref="inviteInputRef"
                  allow-custom
                  type="email"
                  name="emails"
                  accessible-label="E-mailadressen"
                  placeholder="Toevoegen..."
                  :invalid="inviteError ? true : undefined"
                  :error-message="inviteError ? 'invite-email-error' : undefined"
                  @change="onEmailsChange"
                  @input="onEmailsInput"
                ></nldd-token-field>
                <nldd-form-field-help-text>
                  Typ een adres en gebruik een komma om nog een adres in te voeren. Toegang tot '{{ trajectName }}' wordt actief bij de eerste login.
                </nldd-form-field-help-text>
                <nldd-form-field-error-text id="invite-email-error">
                  {{ inviteError }}
                </nldd-form-field-error-text>
              </nldd-form-field>

              <nldd-form-field label="Rol">
                <nldd-toggle-button-group type="radio" size="md" accessible-label="Rol" @change="onRoleChange">
                  <nldd-toggle-button value="contributor" text="Bijdrager" :selected="inviteRole === 'contributor' || undefined"></nldd-toggle-button>
                  <nldd-toggle-button value="owner" text="Beheerder" :selected="inviteRole === 'owner' || undefined"></nldd-toggle-button>
                </nldd-toggle-button-group>
                <nldd-form-field-help-text>
                  Een bijdrager kan wetten en scenario's in dit traject bekijken en bewerken. Een beheerder kan daarnaast ook leden en instellingen van dit traject aanpassen.
                </nldd-form-field-help-text>
              </nldd-form-field>

              <!-- Partial batch (a full success shows the success view above instead). -->
              <div v-if="inviteResult" class="members-info">
                <div v-if="inviteResult.succeeded.length">
                  {{ inviteResult.succeeded.length }} uitnodiging{{ inviteResult.succeeded.length === 1 ? '' : 'en' }} verstuurd. Toegang wordt actief bij de eerste login.
                </div>
                <div v-if="inviteResult.failed.length" class="members-invite-failed">
                  Mislukt:
                  <span v-for="(f, i) in inviteResult.failed" :key="f.email">{{ f.email }} ({{ f.message }}){{ i < inviteResult.failed.length - 1 ? ', ' : '' }}</span>
                </div>
              </div>

              <nldd-form-actions>
                <nldd-button
                  variant="primary"
                  size="md"
                  type="submit"
                  width="full"
                  :text="inviteBusy ? 'Bezig…' : 'Nodig uit'"
                  :disabled="inviteBusy || undefined"
                ></nldd-button>
              </nldd-form-actions>
            </form>
          </nldd-form>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>

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
.members-invite-failed {
  color: var(--nldd-color-text-error, #c62828);
  margin-top: 4px;
}
</style>
