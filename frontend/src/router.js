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
    },
    {
      path: '/editor/:lawId?',
      name: 'editor',
      component: () => import('./EditorApp.vue'),
    },
    {
      path: '/editor.html',
      redirect: (to) => ({
        name: 'editor',
        params: { lawId: to.query.law || undefined },
        query: to.query.article ? { article: to.query.article } : undefined,
      }),
    },
  ],
});

export default router;
