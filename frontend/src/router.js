import { createRouter, createWebHistory } from 'vue-router';
import LibraryApp from './LibraryApp.vue';
import { ensureAuthReady, useAuth } from './composables/useAuth.js';
import { recordLastVisited } from './composables/useLastVisitedRoute.js';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/library' },
    {
      path: '/library/:lawId?/:articleNumber?',
      name: 'library',
      component: LibraryApp,
      meta: { title: 'Bibliotheek' },
    },
    {
      // Documents pane for a traject. Separate URL surface so a deep
      // link to a document does not collide with a law-id slug; the
      // `:docPath(.*)?` catch-all captures hierarchical paths verbatim
      // (e.g. `mvt/concept.md`) — backend uses the same `{*doc_path}`
      // wildcard.
      //
      // Listed BEFORE `editor-traject` so the literal `documents`
      // segment matches this route instead of being captured as a
      // `:lawId`.
      path: '/editor/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/documents/:docPath(.*)?',
      name: 'editor-documents',
      component: () => import('./DocumentsApp.vue'),
      meta: { title: 'Documenten', requiresAuth: true },
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
      // Editor without a traject: read-only view. Useful for browsing
      // a law's editor UI (machine_readable, YAML, scenarios) without
      // committing to a traject. Save actions are disabled (`canEdit`
      // gates them); the user picks a traject via the TrajectMenu to
      // unlock edits, which navigates to `editor-traject`.
      path: '/editor/:lawId?/:articleNumber?',
      name: 'editor',
      component: () => import('./EditorApp.vue'),
      meta: { title: 'Editor', requiresAuth: true },
    },
    {
      path: '/editor.html',
      redirect: (to) => ({
        name: 'editor',
        params: {
          lawId: to.query.law || undefined,
          articleNumber: to.query.article || undefined,
        },
      }),
    },
  ],
});

// Gate any route marked `meta.requiresAuth` on the auth-status check. We
// block the client-side navigation until `/auth/status` has resolved, so
// the target component never mounts until we know the user may enter.
// When OIDC is configured and the user is not authenticated, we trigger
// the SSO redirect here and cancel the navigation \u2014 the previous route
// stays visible until the browser leaves for `/auth/login`, instead of
// flashing the protected UI.
router.beforeEach(async (to) => {
  if (!to.meta.requiresAuth) return true;
  await ensureAuthReady();
  const { authenticated, oidcConfigured, login } = useAuth();
  if (oidcConfigured.value && !authenticated.value) {
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
