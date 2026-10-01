/**
 * The demo's clock during a replay: the moment of the recording, not today.
 *
 * The demo reads the date in a few places (the reference date of a new
 * session, when a case was submitted, the deadline for an objection). A
 * replay on another day would otherwise show other dates than the presenter
 * names, and in a few years other amounts. While the replay runs, `Date`
 * answers with the recorded moment plus the time into the recording.
 */

import { ref } from 'vue';

const RealDate = globalThis.Date;
let installed = null;

/**
 * Bumped when the clock is installed or removed. A computed that formats
 * "today" reads it, so it does not keep the recording's date after a replay
 * (or today's date during one).
 */
export const clockEpoch = ref(0);

/**
 * Make `new Date()` and `Date.now()` return `nowMs()`. `new Date(x)` with an
 * argument is untouched. Calling it again replaces the source.
 */
export function installClock(nowMs) {
  if (installed) {
    installed.source = nowMs;
    return;
  }
  const state = { source: nowMs };
  class ReplayDate extends RealDate {
    constructor(...args) {
      if (args.length) super(...args);
      else super(state.source());
    }

    static now() {
      return state.source();
    }
  }
  installed = state;
  globalThis.Date = ReplayDate;
  clockEpoch.value += 1;
}

export function uninstallClock() {
  if (!installed) return;
  globalThis.Date = RealDate;
  installed = null;
  clockEpoch.value += 1;
}

export function clockInstalled() {
  return !!installed;
}
