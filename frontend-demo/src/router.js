import { createRouter, createWebHistory } from 'vue-router';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'dashboard',
      component: () => import('./views/Dashboard.vue'),
      meta: { title: 'Waar heb ik recht op?' },
    },
    {
      path: '/wet/:lawId',
      name: 'law-detail',
      component: () => import('./views/LawDetail.vue'),
      meta: { title: 'Wet' },
      props: true,
    },
    {
      path: '/wet/:lawId/simulatie',
      name: 'law-simulation',
      component: () => import('./views/LawSimulation.vue'),
      meta: { title: 'Simulatie' },
      props: true,
    },
  ],
});

router.afterEach((to) => {
  const base = 'RegelRecht';
  document.title = to.meta?.title ? `${to.meta.title} — ${base}` : base;
});

export default router;
