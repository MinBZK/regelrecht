/**
 * The presentation deck, shared state for the whole app (module-level refs).
 *
 * The deck tells the story on the left; the live demo answers on the right.
 * An intro or closing slide covers the screen; a slide with a `route` shrinks
 * the deck to a rail, opens that tab, optionally switches the persona and
 * pulses the part of the screen the presenter points at. Slides come from
 * `corpus/demo/demo-config.yaml` (`slides:`), so the story is content, not code.
 *
 * Same pattern as the Begane Grond deck, reduced to what this demo needs.
 */
import { computed, nextTick, ref } from 'vue';

const active = ref(false);
const index = ref(0);
const slidesRef = ref([]);
/**
 * 'zaal' of 'zelfstandig'. Een eigen ref en geen greep in de store, want de
 * store haalt zelf het corpus op en zou dan om deze module heen cirkelen. App
 * houdt hem gelijk aan `state.presentationMode`.
 */
const mode = ref('zaal');

let router = null;
let demo = null;
let listening = false;

const current = computed(() => slidesRef.value[index.value] ?? null);
const total = computed(() => slidesRef.value.length);
/** Full-screen slides: title, statement, closing, or anything without a route. */
const isFull = computed(() => !current.value?.route);
/**
 * Staat het dek in beeld?
 *
 * In de zaal wel op een dia die het scherm dekt (het verhaal), niet op een dia
 * die de demo opent: daar is het scherm van de demo en zou een rail ernaast
 * juist verkleinen wat het publiek moet zien. Zelfstandig staat het dek er
 * altijd, want dan is er niemand die het verhaal vertelt.
 *
 * De presentatie blijft in beide gevallen actief: alleen het renderen stopt,
 * zodat pijltjes en spatie blijven bladeren en `stop()` de toetsen niet
 * loskoppelt.
 */
const visible = computed(() => (mode.value === 'zaal' ? isFull.value : true));

function init({ router: r, demo: d, slides }) {
  if (r) router = r;
  if (d) demo = d;
  if (slides) slidesRef.value = slides;
}

/**
 * De klasse op <html> die de body naast het dek schuift, of juist niet.
 * De body krijgt geen ruimte ingeruimd als het dek het scherm dekt én niet als
 * het er helemaal niet staat; alleen de rail schuift op.
 */
function applyLayout(slide = current.value) {
  // In de zaal schuift de body nooit op: het dek dekt het scherm of het staat
  // er niet. Zelfstandig schuift hij alleen voor een dia met een route, want
  // dan krimpt het dek tot de rail ernaast.
  const full = mode.value === 'zaal' || !slide?.route;
  document.documentElement.classList.toggle('rr-presenting-full', full);
  // Staat het dek er niet, dan neemt het ook de persona niet meer voor zijn
  // rekening, en moet de balk zijn eigen knoppen terugkrijgen.
  const hidden = mode.value === 'zaal' && !!slide?.route;
  document.documentElement.classList.toggle('rr-deck-hidden', hidden);
}

function setMode(m) {
  mode.value = m === 'zelfstandig' ? 'zelfstandig' : 'zaal';
  if (active.value) applyLayout();
}

/** Pulse-highlight what the presenter points at; failures never break the talk. */
function highlight(selector) {
  try {
    const nodes = [...document.querySelectorAll(selector)];
    nodes.forEach((n) => n.classList.add('rr-present-pulse'));
    setTimeout(() => nodes.forEach((n) => n.classList.remove('rr-present-pulse')), 1400);
  } catch {
    /* invalid selector */
  }
}

async function runSlide(i) {
  const s = slidesRef.value[i];
  if (!s) return;
  applyLayout(s);
  // The persona is a function of the slide index: the most recent `profile`
  // at or before this slide, so prev/next/goto agree.
  if (demo) {
    for (let j = i; j >= 0; j -= 1) {
      const p = slidesRef.value[j]?.profile;
      if (p) {
        if (demo.profileKey.value !== p) demo.setProfile(p);
        break;
      }
    }
  }
  if (s.route && router && router.currentRoute.value.path !== s.route) {
    try {
      await router.push(s.route);
    } catch {
      /* redundant navigation */
    }
  }
  await nextTick();
  if (s.highlight) setTimeout(() => highlight(s.highlight), 350);
}

function goTo(i) {
  const n = Math.max(0, Math.min(total.value - 1, i));
  index.value = n;
  return runSlide(n);
}
function next() {
  if (index.value < total.value - 1) return goTo(index.value + 1);
  return stop();
}
function prev() {
  if (index.value > 0) return goTo(index.value - 1);
  return undefined;
}

function onKey(e) {
  if (e.target?.closest?.('input, textarea, select, [contenteditable]')) return;
  // Spatie bedient ook de knop die focus heeft. Zonder dek in beeld (zaalmodus)
  // klikt de presentator in de demo zelf, en dan zou één spatie tegelijk de
  // knop indrukken én een dia verder springen. De pijltjes blijven wel werken,
  // want die doen op een knop niets.
  if (e.key === ' ' && e.target?.closest?.('button, [role="button"], a[href], summary')) return;
  switch (e.key) {
    case 'ArrowRight':
    case ' ':
    case 'PageDown':
      e.preventDefault();
      next();
      break;
    case 'ArrowLeft':
    case 'PageUp':
      e.preventDefault();
      prev();
      break;
    case 'Home':
      goTo(0);
      break;
    case 'End':
      goTo(total.value - 1);
      break;
    case 'Escape':
      stop();
      break;
    case 'f':
      if (document.fullscreenElement) document.exitFullscreen?.();
      else document.documentElement.requestFullscreen?.();
      break;
    default:
  }
}

function start(i = 0) {
  if (!total.value) return;
  active.value = true;
  document.documentElement.classList.add('rr-presenting');
  if (!listening) {
    window.addEventListener('keydown', onKey);
    listening = true;
  }
  goTo(i);
}

function stop() {
  active.value = false;
  document.documentElement.classList.remove('rr-presenting', 'rr-presenting-full', 'rr-deck-hidden');
  if (listening) {
    window.removeEventListener('keydown', onKey);
    listening = false;
  }
}

export function usePresentation() {
  // `mode` zelf gaat er niet uit: de bron daarvan is `state.presentationMode`
  // in de store, en twee plekken om dezelfde waarde te lezen lopen uiteen.
  // Wie wil weten wat het dek doet, leest `visible`.
  return { active, index, current, total, isFull, visible, setMode, slides: slidesRef, init, start, stop, next, prev, goTo };
}
