import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { usePresentation } from './usePresentation.js';

// De presentatie hangt één keydown-luisteraar aan het venster en houdt die
// vast zolang ze actief is. Dat is wat deze tests bewaken: zodra het scherm
// niet meer van de dia's is, mag ze spatie en de pijltjes niet meer afvangen.
//
// De module houdt zijn staat (ook de router) op moduleniveau en `init` schrijft
// alleen wat je meegeeft. Elke test zet daarom in `beforeEach` een verse router
// én dia's, en eindigt met `stop()`. Laat een test de router weg, dan erft hij
// stilzwijgend die van de vorige en bewijst hij niets.

/**
 * Een router-dubbel met alleen wat de presentatie ervan leest en gebruikt.
 * De tabbladen dragen een naam en sommige een optionele parameter, net als in
 * router.js: `/wetten/:lawId?` houdt dezelfde naam als de presentator een wet
 * opent.
 */
const ROUTES = [
  { name: 'home', path: '/' },
  { name: 'wetten', path: '/wetten' },
  { name: 'simulatie', path: '/simulatie' },
  { name: 'zaaksysteem', path: '/zaaksysteem' },
];

/**
 * Onbekend pad geeft `undefined`, net als de echte router: die vangt zoiets op
 * met `/:pathMatch(.*)*`, en die route draagt geen naam.
 */
function nameFor(path) {
  const head = `/${String(path).split('/')[1] ?? ''}`;
  return ROUTES.find((r) => r.path === head)?.name;
}

function fakeRouter(path = '/') {
  const currentRoute = { value: { path, name: nameFor(path) } };
  return {
    currentRoute,
    resolve(to) {
      return { path: to, name: nameFor(to) };
    },
    push(to) {
      currentRoute.value = { path: to, name: nameFor(to) };
      return Promise.resolve();
    },
    /** Wat de presentator zelf doet: klikken en navigeren buiten de dia's om. */
    goTo(to) {
      currentRoute.value = { path: to, name: nameFor(to) };
    },
  };
}

const SLIDES = [
  { kind: 'title', title: 'Opening' },
  { kind: 'demo', title: 'De wetten', route: '/wetten' },
  { kind: 'closing', title: 'Slot' },
];

/** Stuur een keydown naar het venster en meld of iemand hem afving. */
function press(key) {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  document.body.dispatchEvent(event);
  return event.defaultPrevented;
}

/**
 * Een toetsaanslag uit een invoerveld van het ontwerpsysteem, zoals Chrome hem
 * aflevert: het event is geretarget naar de host (`e.target` is de custom
 * element) en de echte `<input>` staat alleen op `composedPath()`.
 *
 * Nagebouwd in plaats van nagespeeld, want happy-dom implementeert die
 * retargeting niet: daar blijft `e.target` gewoon de `<input>`, en dan zou deze
 * test groen staan zonder iets te bewijzen. In Chrome is gemeten dat
 * `e.target` `NLDD-TEXT-FIELD` is en `closest('input')` niets vindt.
 */
function pressFromShadowInput(key) {
  const host = document.createElement('nldd-text-field');
  document.body.append(host);
  const input = document.createElement('input');
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  Object.defineProperty(event, 'target', { value: host });
  Object.defineProperty(event, 'composedPath', { value: () => [input, host, document.body, document.documentElement, document, window] });
  window.dispatchEvent(event);
  host.remove();
  return event.defaultPrevented;
}

describe('usePresentation toetsafvang', () => {
  let p;
  let router;

  beforeEach(() => {
    p = usePresentation();
    router = fakeRouter();
    p.init({ router, slides: SLIDES });
    p.setMode('zaal');
  });

  afterEach(() => {
    p.stop();
  });

  it('bladert op een dia die het scherm dekt', async () => {
    await p.start(0);
    expect(p.visible.value).toBe(true);
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(1);
  });

  it('blijft bladeren op de demo die de dia zelf geopend heeft', async () => {
    await p.start(1);
    // Zaalmodus, dia met een route: het dek staat niet in beeld, maar het
    // scherm is nog van deze dia.
    expect(p.visible.value).toBe(false);
    expect(router.currentRoute.value.path).toBe('/wetten');
    expect(press('ArrowRight')).toBe(true);
    expect(p.index.value).toBe(2);
  });

  it('laat de toetsen los zodra de presentator zelf een ander tabblad opent', async () => {
    await p.start(1);
    router.goTo('/simulatie');

    expect(p.isOnStage()).toBe(false);
    expect(press(' ')).toBe(false);
    expect(press('ArrowRight')).toBe(false);
    expect(press('ArrowLeft')).toBe(false);
    expect(p.index.value).toBe(1);
  });

  it('pakt de toetsen weer op als de presentator terugloopt naar de dia', async () => {
    await p.start(1);
    router.goTo('/simulatie');
    expect(press(' ')).toBe(false);

    router.goTo('/wetten');
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(2);
  });

  it('blijft bladeren als het tabblad zichzelf verdiept', async () => {
    await p.start(1);
    // WettenView opent bij binnenkomst meteen de standaardwet van het profiel
    // en doet `router.replace('/wetten/<lawId>')` (een watch met
    // `immediate: true`), zonder dat de presentator iets aanraakt. Hetzelfde
    // geldt voor ScenariosView. Vergelijken op pad zou het dek dus doof maken
    // op de dia die zojuist zelf dit tabblad opende.
    router.goTo('/wetten/zorgtoeslagwet');

    expect(p.isOnStage()).toBe(true);
    expect(press('ArrowRight')).toBe(true);
    expect(p.index.value).toBe(2);
  });

  it('vangt niets meer af na stop()', async () => {
    await p.start(0);
    p.stop();
    expect(press(' ')).toBe(false);
    expect(p.index.value).toBe(0);
  });

  it('Escape stopt de presentatie ook als je zelf weggelopen bent', async () => {
    await p.start(1);
    router.goTo('/simulatie');
    // De bladertoetsen zijn hier terecht los, maar Escape moet blijven werken:
    // het is de enige toets die een presentatie beeindigt zonder haar uit te
    // lopen, en zonder hem leeft ze onzichtbaar door.
    expect(p.isOnStage()).toBe(false);
    expect(press(' ')).toBe(false);

    press('Escape');
    expect(p.active.value).toBe(false);
    expect(document.documentElement.classList.contains('rr-presenting')).toBe(false);
  });

  it('Escape werkt ook gewoon met het dek in beeld', async () => {
    await p.start(0);
    press('Escape');
    expect(p.active.value).toBe(false);
  });

  it('koppelt de luisteraar los na Escape, zodat de pagina de toetsen terugkrijgt', async () => {
    await p.start(1);
    press('Escape');
    expect(p.active.value).toBe(false);

    // Na Escape is de luisteraar weg: een volgende spatie is weer van de
    // pagina en bladert niets. Dit is wat de gebruiker merkt, en het pint
    // meteen dat `stop()` de luisteraar echt loskoppelt.
    document.documentElement.classList.add('rr-sentinel');
    expect(press(' ')).toBe(false);
    expect(press('Escape')).toBe(false);
    expect(document.documentElement.classList.contains('rr-sentinel')).toBe(true);
    document.documentElement.classList.remove('rr-sentinel');
  });

  it('laat een spatie in een invoerveld van het ontwerpsysteem met rust', async () => {
    await p.start(0);
    // Het dek staat in beeld en vangt spaties af, maar niet deze: die hoort in
    // het veld. `nldd-text-field` zet zijn <input> in een open shadow root, dus
    // het event komt op de host uit en `closest('input')` vindt niets.
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(1);

    expect(pressFromShadowInput(' ')).toBe(false);
    expect(p.index.value).toBe(1); // niet doorgebladerd
  });

  it('houdt de toetsen niet vast op een dia met een route die niet bestaat', async () => {
    // Een typefout in `route:` in demo-config.yaml valt in de catch-all
    // `/:pathMatch(.*)*`, die naar `/` redirect en geen naam draagt. De
    // presentatie belandt dus op home terwijl de dia iets anders noemt: naam
    // noch pad komt overeen, en losgelaten is de veilige kant. Liever een dia
    // die niet bladert dan een spatie die overal in de app verdwijnt.
    const redirecting = fakeRouter();
    redirecting.push = (to) => {
      const landed = nameFor(to) ? to : '/';
      redirecting.currentRoute.value = { path: landed, name: nameFor(landed) };
      return Promise.resolve();
    };
    p.init({ router: redirecting, slides: [{ kind: 'title' }, { kind: 'demo', route: '/tikfout' }] });
    await p.start(1);

    expect(redirecting.currentRoute.value.path).toBe('/');
    expect(p.isOnStage()).toBe(false);
    expect(press(' ')).toBe(false);
  });

  it('houdt zelfstandig de toetsen vast, want dan staat het dek er altijd', async () => {
    p.setMode('zelfstandig');
    await p.start(1);
    router.goTo('/simulatie');

    expect(p.visible.value).toBe(true);
    expect(p.isOnStage()).toBe(true);
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(2);
  });
});
