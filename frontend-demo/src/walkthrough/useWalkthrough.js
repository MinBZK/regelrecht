/**
 * Whether this build carries a recorded walkthrough, loaded once and shared.
 *
 * The home page and the presentation page offer the walkthrough only when
 * there is one; a deploy without the media simply shows no button.
 */
import { shallowRef } from 'vue';
import { loadTimeline } from './timeline.js';

const timeline = shallowRef(null);
let pending = null;

export function useWalkthrough() {
  if (!pending) {
    pending = loadTimeline().then((t) => {
      timeline.value = t;
      return t;
    });
  }
  return { timeline, ready: pending };
}
