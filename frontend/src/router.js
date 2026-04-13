import { createRouter, createWebHistory } from 'vue-router';
import LibraryApp from './LibraryApp.vue';

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
      meta: { title: 'Editor' },
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

router.afterEach((to) => {
  document.title = to.meta.title
    ? `${to.meta.title} \u00b7 RegelRecht`
    : 'RegelRecht';
});

export default router;
