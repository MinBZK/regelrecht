import { ref } from 'vue';
import { apiFetch, apiFetchJson } from '../lib/apiFetch.js';

/**
 * Per-traject member + pending-invite state.
 *
 * Returns a fresh reactive state on every call so multiple dialogs (or
 * the same dialog reopened) don't leak previous data between trajects.
 * The component owning the dialog passes the traject id at load time;
 * the composable handles fetching, invite, role change, removal, and
 * leave.
 */
export function useTrajectMembers() {
  const members = ref([]);
  const pendingInvites = ref([]);
  const callerRole = ref(null);
  const loading = ref(false);
  const error = ref(null);

  async function load(trajectId) {
    // Reset state before the await so a reopen against a different
    // traject can't briefly flash the previous traject's members.
    loading.value = true;
    error.value = null;
    members.value = [];
    pendingInvites.value = [];
    callerRole.value = null;
    try {
      const body = await apiFetchJson(`/api/trajects/${trajectId}`, {
        errorMessage: (status) => `Kon traject niet laden: ${status}`,
      });
      members.value = body.members || [];
      pendingInvites.value = body.pending_invites || [];
      callerRole.value = body.role || null;
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  async function invite(trajectId, email, role) {
    const body = await apiFetchJson(`/api/trajects/${trajectId}/members`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ email, role }),
      // 400 from normalize_email/validate_role comes back with an empty
      // body; surface a specific message instead of "Uitnodigen mislukt: 400".
      errorMessage: (status, reason) =>
        reason ||
        (status === 400 ? 'Ongeldig e-mailadres of rol' : `Uitnodigen mislukt: ${status}`),
    });
    await load(trajectId);
    return body;
  }

  async function updateRole(trajectId, accountId, role) {
    await apiFetch(`/api/trajects/${trajectId}/members/${accountId}`, {
      method: 'PATCH',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ role }),
      // 409 is the backend's atomic "can't demote the last owner" guard - only
      // reachable when demoting an owner to contributor. Surface the workflow.
      errorMessage: (status, body) =>
        status === 409
          ? 'Een traject moet minstens één eigenaar houden. Maak eerst een ander lid eigenaar.'
          : body || `Rol wijzigen mislukt: ${status}`,
    });
    await load(trajectId);
  }

  async function removeMember(trajectId, accountId) {
    await apiFetch(`/api/trajects/${trajectId}/members/${accountId}`, {
      method: 'DELETE',
      errorMessage: (status, body) => body || `Verwijderen mislukt: ${status}`,
    });
    await load(trajectId);
  }

  async function removeInvite(trajectId, email) {
    await apiFetch(
      `/api/trajects/${trajectId}/invites/${encodeURIComponent(email)}`,
      {
        method: 'DELETE',
        errorMessage: (status, body) => body || `Uitnodiging intrekken mislukt: ${status}`,
      },
    );
    await load(trajectId);
  }

  return {
    members,
    pendingInvites,
    callerRole,
    loading,
    error,
    load,
    invite,
    updateRole,
    removeMember,
    removeInvite,
  };
}
