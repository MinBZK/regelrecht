import { createRouter, createWebHistory } from 'vue-router';
import LibraryApp from './LibraryApp.vue';
import { ensureAuthReady, useAuth } from './composables/useAuth.js';

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

router.afterEach((to) => {
  document.title = to.meta.title
    ? `${to.meta.title} \u00b7 RegelRecht`
    : 'RegelRecht';
});

export default router;
