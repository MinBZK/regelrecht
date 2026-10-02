import { createRouter, createWebHistory } from 'vue-router';

const routes = [
  // Beleid eerst: de sessie begint aan de achterkant (de wet en de varianten)
  // en gaat daarna pas naar wat de burger ervan merkt.
  { path: '/', redirect: '/beleid' },
  {
    path: '/burger',
    name: 'burger',
    component: () => import('./views/BurgerView.vue'),
  },
  {
    path: '/beleid',
    name: 'beleid',
    component: () => import('./views/BeleidView.vue'),
  },
  {
    // Op het scherm heet dit "scenario's"; in de code blijft het `personas`,
    // want dat woord staat ook in het gedeelde recordschema en in de andere
    // casus. De oude route blijft werken voor gedeelde links.
    path: '/scenarios',
    name: 'scenarios',
    component: () => import('./views/PersonasView.vue'),
  },
  { path: '/personas', redirect: '/scenarios' },
];

export default createRouter({
  // Waar de app gemonteerd staat: '/' lokaal, '/terugbetaalregimes/' achter het
  // poc-portaal. Vite vult dit uit `base` in vite.config.js.
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});
