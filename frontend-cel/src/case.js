// The layout of the case screen. A case can hold more decisions (an advance,
// a determination, a recovery); the runtime says which decisions there are,
// and per action which decision it now acts on (`decision`, the id of the
// decision gram). This module knows no decision by name: it groups what the
// runtime gives.
import { kindOf, outputText } from './text.js';

// Whether an action belongs to a decision: it recorded it, or it now acts on
// it (a follow-up, a fact that follows it, an amendment of it).
function belongsTo(action, decision) {
  return action.name === decision.action || action.decision === decision.id;
}

// The state of the facts with an amount (such as what is still to be paid):
// the outputs of their trial without a form, so as the case is now.
export function paymentStatus(actions) {
  const out = [];
  for (const a of actions) {
    if (!a.form.some((f) => f.type === 'amount')) continue;
    for (const [name, v] of Object.entries(a.trial?.outputs ?? {})) {
      out.push({ key: a.name + name, action: a.label, name, value: outputText(v, a.types?.[name]) });
    }
  }
  return out;
}

// The case in parts: per decision its actions and payment status, and the
// actions that belong to no decision (yet), such as a decision still to be
// taken or a fact from the course of the case.
export function caseLayout(caseData) {
  const actions = caseData?.actions ?? [];
  const decisions = (caseData?.decisions ?? []).map((d, i) => {
    const own = actions.filter((a) => belongsTo(a, d));
    // The position of the decision in the group: the gram itself only has an id.
    const number = String(i + 1);
    return { ...d, number, ownActions: own, paymentStatus: paymentStatus(own) };
  });
  const other = actions.filter((a) => !decisions.some((d) => belongsTo(a, d)));
  return { decisions, other, paymentStatus: paymentStatus(other) };
}

// The state of an action in a row of the table.
export function statusText(action) {
  if (kindOf(action) !== 'fact' && action.recorded > 0 && !action.available) return 'vastgelegd';
  if (!action.available) return 'nog niet';
  return action.recorded > 0 ? `kan (${action.recorded} keer vastgelegd)` : 'kan';
}

// The kind of an action in (Dutch) words.
const KIND_LABELS = { fact: 'feit', decision: 'besluit', follow_up: 'vervolg' };

export function kindText(action) {
  const k = kindOf(action);
  if (k === 'fact') return KIND_LABELS.fact;
  return `${KIND_LABELS[k] ?? k}, stage ${action.stage}`;
}

// The heading of a decision: which decision, when, and what it amends.
export function decisionHeading(d) {
  const parts = [`besluit ${d.id}`];
  if (d.effective_at) parts.push(`genomen op ${d.effective_at.slice(0, 10)}`);
  if (d.amends) parts.push(`wijzigt besluit ${d.amends}`);
  return parts.join(', ');
}
