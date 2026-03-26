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
  ],
});

export default router;
