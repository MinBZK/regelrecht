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
      // settings menu) and a nested <router-view> for the section bodies.
      // Because the editor and library are children of this one record,
      // switching between them reuses the shell instance — only the nested
      // router-view swaps — so the chrome never rebuilds on a tab switch.
      // Child paths are relative; route names + full paths stay unchanged,
      // so deep links and sectionTarget keep working.
      path: '/',
      component: AppShell,
      children: [
        { path: '', redirect: '/library' },
        {
          // Traject-scoped bibliotheek: the same library UI, but reading
          // through `/api/trajects/{trajectRef}/corpus/...` so the active
          // traject survives a Bibliotheek↔Editor tab switch. Mirrors the
          // `editor-traject` route below — the active traject lives in the
          // URL (per-tab state), never a server session.
          //
          // The `:trajectRef` regex pins the param to `{slug}-{8hex}` so a
          // plain law-id slug like `wet_op_de_zorgtoeslag` does NOT match here — it
          // falls through to the no-traject library below. Same invariant as
          // `editor-traject`: law `$id` slugs must not end in `-{8hex}`.
          //
          // Reads of a traject's corpus require auth (the traject is tied to
          // the user's repo), so this route is gated like `editor-traject`.
          // The user only ever reaches it from an authenticated session.
          path: 'library/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?',
          name: 'library-traject',
          component: LibraryView,
          meta: { title: 'Bibliotheek', requiresAuth: true },
        },
        {
          path: 'library/:lawId?/:articleNumber?',
          name: 'library',
          component: LibraryView,
          meta: { title: 'Bibliotheek' },
        },
        {
          // Traject-scoped editor: full read + write. Per-tab active
          // traject lives in the URL; switching in one tab no longer
          // leaks into another tab's saves. API hangs under
          // `/api/trajects/{trajectRef}/corpus/...`.
          //
          // The `:trajectRef` regex pins the param to `{slug}-{8hex}` so a
          // plain law-id slug like `wet_op_de_zorgtoeslag` does NOT match this
          // route — it falls through to the no-traject editor below.
          //
          // **Invariant**: law `$id` slugs must not match this regex (i.e.
          // they must not end in `-{8hex}`). Today every harvested $id uses
          // underscores (e.g. `wet_op_de_zorgtoeslag`) which are excluded
          // from the character class, so the collision is structurally
          // impossible. If a future harvester ever emits hyphenated ids, a
          // schema check (or this regex tightened to require a leading
          // word from the slug, not just hex chars) must be added.
          path: 'editor/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?',
          name: 'editor-traject',
          component: () => import('./EditorView.vue'),
          meta: { title: 'Editor', requiresAuth: true },
        },
        {
          // Editor without a traject: read-only view. Useful for browsing
          // a law's editor UI (machine_readable, YAML, scenarios) without
          // committing to a traject. Save actions are disabled (`canEdit`
          // gates them); the user picks a traject via the TrajectMenu to
          // unlock edits, which navigates to `editor-traject`.
          path: 'editor/:lawId?/:articleNumber?',
          name: 'editor',
          component: () => import('./EditorView.vue'),
          meta: { title: 'Editor', requiresAuth: true },
        },
      ],
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

// Note: document.title is owned by the route components (LibraryView, EditorView)
// via watchEffect \u2014 they reflect law + article state. router.afterEach used to
// set a static title here, but it ran AFTER the component's reactive update
// (vue's effect flush is sync; afterEach is one microtask later), so a
// tab-switch or article-select would set "Editor: Art. 5 \u00b7 ..." and then
// immediately get clobbered back to "Editor \u00b7 RegelRecht". Letting the
// components own the title avoids the race.

export default router;
