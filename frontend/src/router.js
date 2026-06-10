import { createRouter, createWebHistory } from 'vue-router';
import LibraryApp from './LibraryApp.vue';
import { ensureAuthReady, useAuth } from './composables/useAuth.js';
import { recordLastVisited } from './composables/useLastVisitedRoute.js';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/library' },
    {
      // Traject-scoped bibliotheek: the same library UI, but reading
      // through `/api/trajects/{trajectRef}/corpus/...` so the active
      // traject survives a Bibliotheek↔Editor tab switch. Mirrors the
      // `editor-traject` route below — the active traject lives in the
      // URL (per-tab state), never a server session.
      //
      // The `:trajectRef` regex pins the param to `{slug}-{8hex}` so a
      // plain law-id slug like `zorgtoeslagwet` does NOT match here — it
      // falls through to the no-traject library below. Same invariant as
      // `editor-traject`: law `$id` slugs must not end in `-{8hex}`.
      //
      // Reads of a traject's corpus require auth (the traject is tied to
      // the user's repo), so this route is gated like `editor-traject`.
      // The user only ever reaches it from an authenticated session.
      path: '/library/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?',
      name: 'library-traject',
      component: LibraryApp,
      meta: { title: 'Bibliotheek', requiresAuth: true },
    },
    {
      path: '/library/:lawId?/:articleNumber?',
      name: 'library',
      component: LibraryApp,
      meta: { title: 'Bibliotheek' },
    },
    {
      // Traject-scoped editor: full read + write. Per-tab active
      // traject lives in the URL; switching in one tab no longer
      // leaks into another tab's saves. API hangs under
      // `/api/trajects/{trajectRef}/corpus/...`.
      //
      // The `:trajectRef` regex pins the param to `{slug}-{8hex}` so a
      // plain law-id slug like `zorgtoeslagwet` does NOT match this
      // route — it falls through to the no-traject editor below.
      //
      // **Invariant**: law `$id` slugs must not match this regex (i.e.
      // they must not end in `-{8hex}`). Today every harvested $id uses
      // underscores (e.g. `wet_op_de_zorgtoeslag`) which are excluded
      // from the character class, so the collision is structurally
      // impossible. If a future harvester ever emits hyphenated ids, a
      // schema check (or this regex tightened to require a leading
      // word from the slug, not just hex chars) must be added.
      path: '/editor/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?',
      name: 'editor-traject',
      component: () => import('./EditorApp.vue'),
      meta: { title: 'Editor', requiresAuth: true },
    },
    {
      // Nieuw traject aanmaken — eigen pagina met het gedeelde
      // aanmaakformulier (TrajectCreateForm). Statisch segment, dus
      // vue-router rankt dit boven de param-routes hieronder.
      path: '/editor/nieuw-traject',
      name: 'editor-nieuw-traject',
      component: () => import('./TrajectCreateApp.vue'),
      meta: { title: 'Nieuw traject', requiresAuth: true },
    },
    {
      // De editor vereist een traject. De kale /editor is de
      // trajectkeuze-pagina: kies een bestaand traject of maak er een
      // aan; daarna ga je door naar `editor-traject`. Een eventueel
      // meegegeven wet (query `law`/`article`, gezet door de redirect
      // hieronder) opent na de keuze direct in de editor.
      path: '/editor',
      name: 'editor',
      component: () => import('./TrajectChooserApp.vue'),
      meta: { title: 'Kies een traject', requiresAuth: true },
    },
    {
      // Editor-links zonder traject (de vroegere read-only editor):
      // er is geen editor zonder traject meer. Door naar de
      // keuzepagina, met de wet als query zodat die na de keuze opent.
      path: '/editor/:lawId/:articleNumber?',
      redirect: (to) => ({
        name: 'editor',
        query: {
          law: to.params.lawId,
          article: to.params.articleNumber || undefined,
        },
      }),
    },
    {
      path: '/editor.html',
      redirect: (to) => ({
        name: 'editor',
        query: {
          law: to.query.law || undefined,
          article: to.query.article || undefined,
        },
      }),
    },
  ],
});

// Gate any route marked `meta.requiresAuth` on the auth-status check. We
// block the client-side navigation until `/auth/status` has resolved, so
// the target component never mounts until we know the user may enter.
// Unauthenticated users are always sent to `/auth/login` and the
// navigation is cancelled \u2014 the previous route stays visible until the
// browser leaves, instead of flashing the protected UI. Deliberately NOT
// conditional on `oidcConfigured`: the editor must never open without
// login, including environments without OIDC (there `/auth/login` either
// serves the dev login or surfaces a backend error). A failed
// /auth/status check leaves `authenticated` false and thus fails closed.
router.beforeEach(async (to) => {
  if (!to.meta.requiresAuth) return true;
  await ensureAuthReady();
  const { authenticated, login } = useAuth();
  if (!authenticated.value) {
    // Pass the intended destination explicitly: inside beforeEach, the
    // client-side navigation has not committed yet, so window.location
    // still reflects the source route (e.g. /library). Without this, the
    // user would land back on the source route after SSO instead of the
    // page they originally clicked.
    login(to.fullPath);
    return false;
  }
  return true;
});

// Track the last fullPath per route name so the Bibliotheek/Editor tab
// switch can restore the user's prior position in each section.
router.afterEach((to) => {
  recordLastVisited(to.name, to.fullPath);
});

// Note: document.title is owned by the route components (LibraryApp, EditorApp)
// via watchEffect \u2014 they reflect law + article state. router.afterEach used to
// set a static title here, but it ran AFTER the component's reactive update
// (vue's effect flush is sync; afterEach is one microtask later), so a
// tab-switch or article-select would set "Editor: Art. 5 \u00b7 ..." and then
// immediately get clobbered back to "Editor \u00b7 RegelRecht". Letting the
// components own the title avoids the race.

export default router;
