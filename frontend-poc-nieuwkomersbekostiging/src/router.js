import { createRouter, createWebHistory } from 'vue-router';

const routes = [
  { path: '/', redirect: '/beleid' },
  { path: '/burger', redirect: '/casussen' },
  {
    path: '/beleid',
    name: 'beleid',
    component: () => import('./views/BeleidView.vue'),
  },
  {
    path: '/casussen',
    name: 'casussen',
    component: () => import('./views/CasussenView.vue'),
  },
  {
    path: '/school',
    name: 'school',
    component: () => import('./views/SchoolView.vue'),
  },
];

export default createRouter({
  // Waar de app gemonteerd staat: '/' lokaal, '/nieuwkomersbekostiging/' achter
  // het poc-portaal. Vite vult dit uit `base` in vite.config.js.
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});
