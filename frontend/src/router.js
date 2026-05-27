import { createRouter, createWebHistory } from 'vue-router';
import LibraryApp from './LibraryApp.vue';
import { ensureAuthReady, useAuth } from './composables/useAuth.js';
import { recordLastVisited } from './composables/useLastVisitedRoute.js';

// Traject ref shape: `{slug}-{8hex}` where the trailing 8 hex chars are
// the lookup key against the trajects table (slug is cosmetic — see
// `resolve_traject_ref` in editor-api). Loose enough to accept the
// runtime-generated slug, strict enough to disambiguate from a bare
// law-id slug so an old-shape `/editor/{lawId}` bookmark falls through
// to the legacy redirect below.
const TRAJECT_REF_RE = /^[a-z0-9-]+-[0-9a-f]{8}$/i;

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
      // Per-tab active traject lives in the URL: each open tab carries
      // its own `trajectRef`, so a switch in one tab no longer leaks
      // into another tab's saves. The matching API routes hang under
      // `/api/trajects/{trajectRef}/corpus/...` (see editor-api main.rs).
      path: '/editor/:trajectRef/:lawId?/:articleNumber?',
      name: 'editor',
      component: () => import('./EditorApp.vue'),
      meta: { title: 'Editor', requiresAuth: true },
      beforeEnter: (to) => {
        // Legacy bookmark `/editor/{lawId}` (no traject) — interpret
        // `trajectRef` as the law id and redirect to the global library
        // view. Users can re-open in a traject via the menu.
        if (!TRAJECT_REF_RE.test(to.params.trajectRef)) {
          return {
            name: 'library',
            params: {
              lawId: to.params.trajectRef,
              articleNumber: to.params.lawId,
            },
          };
        }
      },
    },
    {
      path: '/editor.html',
      // Legacy query-string entry point — without a traject we can't
      // land in the editor, so route to the library view of the
      // requested law instead.
      redirect: (to) => ({
        name: 'library',
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
