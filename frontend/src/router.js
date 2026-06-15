import { createRouter, createWebHistory } from 'vue-router';
import AppShell from './AppShell.vue';
import LibraryView from './LibraryView.vue';
import { ensureAuthReady, useAuth } from './composables/useAuth.js';
import { recordLastVisited } from './composables/useLastVisitedRoute.js';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      // Persistent shell: holds the shared chrome (toolbars, tab-bar,
      // settings menu, traject-switcher) and a nested <router-view> for the
      // section bodies. Editor and library are children of this one record,
      // so switching between them reuses the shell instance — only the nested
      // router-view swaps — and the chrome never rebuilds on a tab switch.
      path: '/',
      component: AppShell,
      children: [
        { path: '', redirect: '/library' },
        {
          // Traject-scoped bibliotheek: the same library UI, but reading
          // through `/api/trajects/{trajectRef}/corpus/...` so the active
          // traject survives a Bibliotheek↔Editor tab switch. The active
          // traject lives in the URL (per-tab state), never a server session.
          //
          // The `:trajectRef` regex pins the param to `{slug}-{8hex}` so a
          // plain law-id slug like `wet_op_de_zorgtoeslag` does NOT match
          // here — it falls through to the no-traject library below.
          path: 'library/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?',
          name: 'library-traject',
          component: LibraryView,
          meta: { title: 'Bibliotheek', requiresAuth: true },
        },
        {
          // No-traject bibliotheek: public, global browse (no auth).
          path: 'library/:lawId?/:articleNumber?',
          name: 'library',
          component: LibraryView,
          meta: { title: 'Bibliotheek' },
        },
        {
          // Traject-scoped editor: full read + write. Per-tab active traject
          // lives in the URL. API hangs under `/api/trajects/{trajectRef}/...`.
          //
          // **Invariant**: law `$id` slugs must not match the `{slug}-{8hex}`
          // regex (they use underscores, e.g. `wet_op_de_zorgtoeslag`), so a
          // plain law id can never be mistaken for a traject ref.
          path: 'editor/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?',
          name: 'editor-traject',
          component: () => import('./EditorView.vue'),
          meta: { title: 'Editor', requiresAuth: true },
        },
        {
          // Nieuw traject aanmaken — eigen pagina met het gedeelde
          // aanmaakformulier (TrajectCreateForm). Statisch segment, dus
          // vue-router rankt dit boven de param-routes hieronder.
          path: 'editor/nieuw-traject',
          name: 'editor-nieuw-traject',
          component: () => import('./TrajectCreateView.vue'),
          meta: { title: 'Nieuw traject', requiresAuth: true },
        },
        {
          // De editor vereist een traject. De kale /editor is de
          // trajectkeuze-pagina: kies een bestaand traject of maak er een
          // aan; daarna ga je door naar `editor-traject`. Een meegegeven wet
          // (query `law`/`article`, gezet door de redirect hieronder) opent
          // na de keuze direct in de editor.
          path: 'editor',
          name: 'editor',
          component: () => import('./TrajectChooserView.vue'),
          meta: { title: 'Kies een traject', requiresAuth: true },
        },
        {
          // Editor-links zonder traject (de vroegere read-only editor): er is
          // geen editor zonder traject meer. Door naar de keuzepagina, met de
          // wet als query zodat die na de keuze opent.
          path: 'editor/:lawId/:articleNumber?',
          redirect: (to) => ({
            name: 'editor',
            query: {
              law: to.params.lawId,
              article: to.params.articleNumber || undefined,
            },
          }),
        },
      ],
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
// block the client-side navigation until `/auth/status` has resolved, so the
// target component never mounts until we know the user may enter.
// Unauthenticated users are always sent to `/auth/login` and the navigation
// is cancelled — the previous route stays visible until the browser leaves,
// instead of flashing the protected UI. Deliberately NOT conditional on
// `oidcConfigured`: the editor must never open without login, including
// environments without OIDC (there `/auth/login` either serves the dev login
// or surfaces a backend error). A failed /auth/status check leaves
// `authenticated` false and thus fails closed.
router.beforeEach(async (to) => {
  if (!to.meta.requiresAuth) return true;
  await ensureAuthReady();
  const { authenticated, login } = useAuth();
  if (!authenticated.value) {
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

// document.title is owned by the route components (they reflect law + article
// state) via watchEffect — see the note that used to live here on main.

export default router;
