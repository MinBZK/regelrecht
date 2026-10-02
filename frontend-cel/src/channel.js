// Channels and roles as GET /api/processes gives them for a process
// (declared in the actor's policy, RFC-047). No channel is fixed here: the
// screens build their fields, labels and choices from this description.

// The roles of a process as a list: [{id, label, channel, routes}], in the
// order of the runtime.
export function rolesOf(process) {
  return Object.entries(process?.roles ?? {}).map(([id, r]) => ({ id, ...r }));
}

// The first screen of a role: what its first route group shows. A role
// without a screen has no start screen (null): the chronicle of the cell is
// not open, only a handler sees it.
export function startScreen(process, role) {
  const routes = process?.roles?.[role]?.routes ?? [];
  if (routes.includes('portal') && process.portal) return 'possibilities';
  if (routes.includes('handling') && process.handling) return 'worklist';
  if (routes.includes('counter') && process.counter) return 'counter';
  return null;
}

// The channels the portal logs in through: those of the roles with routes
// `portal`, each once, as [{id, role, ...channel}]. The counter identifies
// the applicant with the fields of such a channel.
// `role` is the label of the first portal role of the channel: that is how
// the counter names the choice.
export function portalChannels(process) {
  const out = [];
  for (const r of rolesOf(process)) {
    const c = process.channels?.[r.channel];
    if (r.routes.includes('portal') && c && !out.some((x) => x.id === r.channel)) {
      out.push({ id: r.channel, role: r.label ?? r.id, ...c });
    }
  }
  return out;
}

// Who is logged in, as text: the values of the fields in the order of the
// channel, and the label of the role.
export function sessionText(process, session) {
  if (!session) return '';
  const fields = process?.channels?.[session.channel]?.fields ?? [];
  const values = fields.map((f) => session.fields?.[f.name]).filter(Boolean);
  const role = process?.roles?.[session.role]?.label ?? session.role;
  return [...values, role].join(', ');
}

// A value per field of a channel, empty or taken from an example.
export function emptyFields(channel, source = {}) {
  return Object.fromEntries((channel?.fields ?? []).map((f) => [f.name, source[f.name] ?? '']));
}
