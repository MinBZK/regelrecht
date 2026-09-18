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

/**
 * Luistert het dek nog naar de toetsen?
 *
 * Staat het dek in beeld, dan hoort het toetsenbord erbij. Staat het er niet
 * (zaalmodus op een dia die de demo opent), dan alleen zolang het scherm nog
 * van díe dia is: de presentator klikt in de demo die de dia geopend heeft en
 * bladert ondertussen door. Loopt hij daarna zelf naar een ander tabblad, dan
 * gaat de presentatie nergens meer over en moeten spatie en de pijltjes terug
 * naar de pagina. Anders drukt spatie op de voorgrond niets in en springt er
 * onzichtbaar een dia verder.
 *
 * Een functie en geen `computed`: dit wordt gelezen op het moment van een
 * toetsaanslag, niet in een template. Een computed zou de route cachen en is
 * daarmee juist onbetrouwbaar op het enige moment dat telt.
 */
function isOnStage() {
  if (!active.value) return false;
  if (visible.value) return true;
  const slideRoute = current.value?.route;
  // Geen router (init is nog niet langsgekomen): dan valt niet vast te stellen
  // waar we zijn, en houdt het dek de toetsen niet vast. Dat is de veilige
  // kant: onzichtbaar bladeren is precies wat hier misging.
  if (!slideRoute || !router) return false;
  // Op het tabblad vergelijken en niet op het pad. `/wetten/:lawId?`,
  // `/scenarios/:featurePath(.*)?` en `/zaaksysteem/:caseId?` verdiepen hun
  // eigen pad: WettenView en ScenariosView doen bij binnenkomst meteen een
  // `router.replace` naar de standaardwet of -feature van het profiel, nog
  // voordat de presentator iets aanraakt. Op het pad vergelijken zou het dek
  // dus doof maken op precies de dia die dat tabblad zojuist opende.
  const here = router.currentRoute?.value;
  const target = router.resolve?.(slideRoute);
  if (target?.name && here?.name) return here.name === target.name;
  return here?.path === slideRoute;
}

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

/**
 * Waar kwam deze toetsaanslag vandaan?
 *
 * Niet `e.target.closest(...)`, want de invoervelden van het ontwerpsysteem
 * (`nldd-text-field`, `nldd-date-field`, ...) zetten hun `<input>` in een open
 * shadow root. Het event retarget dan naar de host, en `closest('input')` op
 * die host vindt niets: gemeten in Chrome is `e.target` `NLDD-TEXT-FIELD` en
 * staat de `<input>` alleen op `composedPath()`. Zonder dit liep een spatie in
 * een bedragveld niet het veld in maar bladerde hij een dia verder.
 *
 * `composedPath()` loopt dwars door shadow-grenzen heen, dus daar staat alles
 * tussen de echte `<input>` en `window`. Lukt dat niet (oudere browser,
 * gesynthetiseerd event), dan valt hij terug op `closest`.
 */
function pathMatches(e, selector) {
  const path = e.composedPath?.();
  if (Array.isArray(path) && path.length) {
    return path.some((n) => n?.matches?.(selector));
  }
  return !!e.target?.closest?.(selector);
}

function onKey(e) {
  if (pathMatches(e, 'input, textarea, select, [contenteditable]')) return;
  // Escape blijft altijd de noodrem, ook als het dek niet in beeld staat. Het
  // is de enige toets die een presentatie beeindigt zonder haar uit te lopen,
  // en juist off-stage moet dat kunnen: dan leeft ze onzichtbaar door.
  // Bewust vóór de poort hieronder, want die is precies de toestand waarin je
  // eruit wilt.
  if (e.key === 'Escape' && active.value) {
    stop();
    return;
  }
  // De bladertoetsen gaan wél door de poort: is het scherm niet meer van de
  // presentatie, dan vangt ze niets meer af.
  if (!isOnStage()) return;
  // Spatie bedient ook de knop die focus heeft. Zonder dek in beeld (zaalmodus)
  // klikt de presentator in de demo zelf, en dan zou één spatie tegelijk de
  // knop indrukken én een dia verder springen. De pijltjes blijven wel werken,
  // want die doen op een knop niets.
  if (e.key === ' ' && pathMatches(e, 'button, [role="button"], a[href], summary')) return;
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
    // Escape staat hierboven, vóór de poort.
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
  return { active, index, current, total, isFull, visible, isOnStage, setMode, slides: slidesRef, init, start, stop, next, prev, goTo };
}
