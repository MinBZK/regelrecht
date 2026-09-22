import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { usePresentation } from './usePresentation.js';
import { adoptLocale } from '../i18n/index.js';

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

/**
 * Een route zoals de echte router hem teruggeeft, inclusief `meta.page`: het
 * dek vergelijkt daarop, omdat dezelfde pagina per taal een eigen routenaam
 * heeft (`wetten` en `wetten:en`) en `meta.page` is wat die twee delen.
 */
/** De Engelse slugs van de paden die deze tests aanraken. */
const EN_PATHS = { wetten: '/laws', simulatie: '/simulation', zaaksysteem: '/cases' };

function routeFor(path) {
  // Een Engels pad hoort bij dezelfde pagina als zijn Nederlandse tegenhanger:
  // dat is precies wat `meta.page` uitdrukt, en waar het dek op vergelijkt.
  const enPage = Object.keys(EN_PATHS).find((page) => path === `/en${EN_PATHS[page]}`);
  if (enPage) return { path, name: `${enPage}:en`, meta: { page: enPage, locale: 'en' } };
  if (path === '/en') return { path, name: 'home:en', meta: { page: 'home', locale: 'en' } };
  const name = nameFor(path);
  return { path, name, meta: name ? { page: name, locale: 'nl' } : {} };
}

function fakeRouter(path = '/') {
  const currentRoute = { value: routeFor(path) };
  return {
    currentRoute,
    /**
     * De echte router wordt zowel met een pad als met `{ name }` aangeroepen;
     * het dek doet dat laatste om een dia-pad naar de actieve taal te brengen.
     */
    resolve(to) {
      if (to && typeof to === 'object') {
        const en = String(to.name).endsWith(':en');
        const hit = ROUTES.find((r) => r.name === String(to.name).replace(/:en$/, ''));
        if (!hit) return { path: '/', name: undefined, meta: {} };
        if (!en) return routeFor(hit.path);
        const path = hit.path === '/' ? '/en' : `/en${EN_PATHS[hit.name] ?? hit.path}`;
        return { path, name: `${hit.name}:en`, meta: { page: hit.name, locale: 'en' } };
      }
      return routeFor(to);
    },
    push(to) {
      currentRoute.value = routeFor(to);
      return Promise.resolve();
    },
    /** Wat de presentator zelf doet: klikken en navigeren buiten de dia's om. */
    goTo(to) {
      currentRoute.value = routeFor(to);
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

describe('usePresentation in het Engels', () => {
  let p;
  let router;

  beforeEach(() => {
    p = usePresentation();
    router = fakeRouter();
    p.init({ router, slides: SLIDES });
    p.setMode('zaal');
    adoptLocale('en');
  });

  afterEach(() => {
    p.stop();
    adoptLocale('nl');
  });

  it('opent het Engelse tabblad voor een dia met een Nederlands pad', async () => {
    // `route: /wetten` staat zo in demo-config.yaml, want dat bestand gaat over
    // dia's en hoort de routetabel van elke taal niet te kennen. Wie het dek in
    // het Engels draait hoort wel op /en/laws te landen.
    await p.start(1);
    expect(router.currentRoute.value.path).toBe('/en/laws');
  });

  it('houdt de toetsen vast op het Engelse tabblad dat de dia opende', async () => {
    // Het dek vergelijkt op de pagina en niet op de routenaam; zou het dat wel
    // doen, dan was het doof op precies de dia die dit tabblad zojuist opende.
    await p.start(1);
    // Zaalmodus, dia met een route: het dek staat niet in beeld, maar het
    // scherm is nog van deze dia.
    expect(p.visible.value).toBe(false);
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(2);
  });
});

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
      redirecting.currentRoute.value = routeFor(landed);
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
