import { createRouter, createWebHistory } from 'vue-router';

// One route per workspace tab. Every view is kept alive by App.vue's
// <keep-alive>, so switching tabs during a presentation never loses state
// (opened law tabs, a running scenario, an expanded tile).
const routes = [
  // `/` is de landingspagina: wie de demo opent zonder te weten wat het is,
  // leest daar wat er te zien valt en start van daaruit. De presentatie had
  // dit adres eerder zelf en begon bij binnenkomst vanzelf, wat een bezoeker
  // midden in een verhaal zette dat hij niet had aangevraagd.
  { path: '/', name: 'home', component: () => import('./views/HomeView.vue') },
  { path: '/presentatie', name: 'presentatie', component: () => import('./views/PresentatieView.vue') },
  { path: '/wetten/:lawId?', name: 'wetten', component: () => import('./views/WettenView.vue') },
  { path: '/graaf', name: 'graaf', component: () => import('./views/GraafView.vue') },
  { path: '/scenarios/:featurePath(.*)?', name: 'scenarios', component: () => import('./views/ScenariosView.vue') },
  { path: '/simulatie', name: 'simulatie', component: () => import('./views/SimulatieView.vue') },
  { path: '/portaal', name: 'portaal', component: () => import('./views/PortaalView.vue') },
  { path: '/zaaksysteem/:caseId?', name: 'zaaksysteem', component: () => import('./views/ZaaksysteemView.vue') },
  { path: '/:pathMatch(.*)*', redirect: '/' },
];

export default createRouter({
  history: createWebHistory(),
  routes,
});
