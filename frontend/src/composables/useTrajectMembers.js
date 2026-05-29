import { ref } from 'vue';

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
      const resp = await fetch(`/api/trajects/${trajectId}`);
      if (!resp.ok) {
        throw new Error(`Kon traject niet laden: ${resp.status}`);
      }
      const body = await resp.json();
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
    const resp = await fetch(`/api/trajects/${trajectId}/members`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ email, role }),
    });
    if (!resp.ok) {
      const reason = await resp.text();
      // 400 from normalize_email/validate_role comes back with an empty
      // body; surface a specific message instead of "Uitnodigen mislukt: 400".
      if (resp.status === 400) {
        throw new Error(reason || 'Ongeldig e-mailadres of rol');
      }
      throw new Error(reason || `Uitnodigen mislukt: ${resp.status}`);
    }
    const body = await resp.json();
    await load(trajectId);
    return body;
  }

  async function updateRole(trajectId, accountId, role) {
    const resp = await fetch(
      `/api/trajects/${trajectId}/members/${accountId}`,
      {
        method: 'PATCH',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ role }),
      },
    );
    if (!resp.ok) {
      throw new Error((await resp.text()) || `Rol wijzigen mislukt: ${resp.status}`);
    }
    await load(trajectId);
  }

  async function removeMember(trajectId, accountId) {
    const resp = await fetch(
      `/api/trajects/${trajectId}/members/${accountId}`,
      { method: 'DELETE' },
    );
    if (!resp.ok) {
      throw new Error((await resp.text()) || `Verwijderen mislukt: ${resp.status}`);
    }
    await load(trajectId);
  }

  async function removeInvite(trajectId, email) {
    const resp = await fetch(
      `/api/trajects/${trajectId}/invites/${encodeURIComponent(email)}`,
      { method: 'DELETE' },
    );
    if (!resp.ok) {
      throw new Error((await resp.text()) || `Uitnodiging intrekken mislukt: ${resp.status}`);
    }
    await load(trajectId);
  }

  async function leaveTraject(trajectId) {
    const resp = await fetch(`/api/trajects/${trajectId}/leave`, {
      method: 'POST',
    });
    if (!resp.ok) {
      throw new Error((await resp.text()) || `Verlaten mislukt: ${resp.status}`);
    }
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
    leaveTraject,
  };
}
