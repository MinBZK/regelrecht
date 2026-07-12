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
      // so switching between them reuses the shell instance - only the nested
      // router-view swaps - and the chrome never rebuilds on a tab switch.
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
          // here - it falls through to the no-traject library below.
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
          // De kale /editor vereist nog steeds een traject: door naar de
          // chooser met sectie=editor (meegegeven query blijft behouden). De
          // naam 'editor' blijft bestaan zodat alle bestaande
          // `{ name: 'editor' }`-navigaties (Editor-tab, sectionTarget,
          // redirects) hier doorheen naar de chooser lopen.
          path: 'editor',
          name: 'editor',
          redirect: (to) => ({ name: 'trajecten', query: { sectie: 'editor', ...to.query } }),
        },
        {
          // Editor-links zonder traject (de vroegere read-only editor): er is
          // geen editor zonder traject meer. Door naar de keuzepagina, met de
          // wet als query zodat die na de keuze opent.
          //
          // Staat bewust NA `editor-traject`: een traject-ref-URL
          // ({slug}-{8hex}) moet de traject-route matchen, niet deze redirect.
          // De declaratievolgorde is hier de scheidsrechter.
          path: 'editor/:lawId/:articleNumber?',
          redirect: (to) => ({
            name: 'trajecten',
            query: {
              sectie: 'editor',
              law: to.params.lawId,
              article: to.params.articleNumber || undefined,
            },
          }),
        },
      ],
    },
    {
      // Trajectchooser - sectie-neutrale URL (library|editor via `sectie`,
      // default editor) zodat zowel de bibliotheek als de editor 'm gebruiken;
      // `law`/`article` dragen de beoogde bestemming mee. Top-level route (geen
      // AppShell-child), zoals werkdocumenten: geen app-chrome, de pagina draagt
      // z'n eigen top-title-bar met terugknop naar de bibliotheek.
      path: '/trajecten',
      name: 'trajecten',
      component: () => import('./TrajectChooserView.vue'),
      meta: { title: 'Trajecten', requiresAuth: true },
    },
    {
      // Nieuw traject aanmaken - eigen pagina met het gedeelde aanmaakformulier
      // (TrajectCreateForm). Ook top-level (geen app-chrome). Het statische pad
      // `/editor/nieuw-traject` scoort boven de dynamische `/editor/...`-routes
      // in de AppShell, dus een traject-ref of wet-id matcht deze nooit.
      path: '/editor/nieuw-traject',
      name: 'editor-nieuw-traject',
      component: () => import('./TrajectCreateView.vue'),
      meta: { title: 'Nieuw traject', requiresAuth: true },
    },
    {
      // Harvester-admin "Corpusinwinning" section - the merged harvester dashboard.
      // Top-level route (sibling of AppShell, not nested) so it carries its own
      // chrome (HarvesterView), with the two sub-screens as nested children -
      // mirroring the original standalone admin dashboard. Gated on any
      // harvester-* role via `meta.requiresRole` (checked in `beforeEach`);
      // write actions inside are enforced server-side by the harvester-admin
      // API. Child routes inherit this record's meta.
      path: '/harvesting',
      component: () => import('./harvester/HarvesterView.vue'),
      meta: {
        title: 'Harvester',
        requiresAuth: true,
        requiresRole: [
          'harvester-reader',
          'harvester-writer',
          'harvester-admin',
          'regelrecht-admin',
        ],
      },
      children: [
        { path: '', redirect: '/harvesting/overview' },
        {
          path: 'overview',
          name: 'overview',
          component: () => import('./harvester/views/OverviewView.vue'),
        },
        {
          path: 'law-entries',
          name: 'law-entries',
          component: () => import('./harvester/views/LawEntriesView.vue'),
        },
        {
          path: 'jobs',
          name: 'jobs',
          component: () => import('./harvester/views/JobsView.vue'),
        },
        {
          path: 'untranslatables',
          name: 'untranslatables',
          component: () => import('./harvester/views/UntranslatablesView.vue'),
        },
      ],
    },
    {
      // Standalone full-page werkdocumenten editor, opened in a new tab from
      // the in-sheet editor ("Open in nieuw tabblad"). Deliberately a top-level
      // route, NOT a child of AppShell: it carries its own minimal top bar
      // instead of the app chrome, giving the document a full navigation-split-
      // view (list + editor). `:trajectRef` is pinned to `{slug}-{8hex}` like
      // the other traject routes; `:docPath(.*)` captures nested document paths
      // (slashes allowed) and is optional so the bare page opens on the list.
      path: '/werkdocumenten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:docPath(.*)?',
      name: 'werkdocumenten',
      component: () => import('./WerkdocumentenView.vue'),
      meta: { title: 'Werkdocumenten', requiresAuth: true },
    },
    {
      path: '/editor.html',
      redirect: (to) => ({
        name: 'trajecten',
        query: {
          sectie: 'editor',
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
// is cancelled - the previous route stays visible until the browser leaves,
// instead of flashing the protected UI. Deliberately NOT conditional on
// `oidcConfigured`: the editor must never open without login, including
// environments without OIDC (there `/auth/login` either serves the dev login
// or surfaces a backend error). A failed /auth/status check leaves
// `authenticated` false and thus fails closed.
router.beforeEach(async (to) => {
  if (!to.meta.requiresAuth) return true;
  await ensureAuthReady();
  const { authenticated, hasAnyRole, login } = useAuth();
  if (!authenticated.value) {
    login(to.fullPath);
    return false;
  }
  // Role-gated routes (e.g. the harvester-admin Corpusinwinning section): an
  // authenticated user lacking the required role is redirected to the
  // library rather than bounced through login (which would loop, since
  // logging in again yields the same role set). `requiresRole` is a list of
  // acceptable roles; holding any one grants access. `meta` is merged across
  // matched records, so a child inherits its parent's `requiresRole`.
  if (to.meta.requiresRole && !hasAnyRole(to.meta.requiresRole)) {
    return { path: '/library' };
  }
  return true;
});

// Track the last fullPath per route name so the Bibliotheek/Editor tab
// switch can restore the user's prior position in each section.
router.afterEach((to) => {
  recordLastVisited(to.name, to.fullPath);
});

// document.title is owned by the route components (they reflect law + article
// state) via watchEffect - see the note that used to live here on main.

export default router;
