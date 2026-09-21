import '@nldd/design-system';
import '@nldd/design-system/styles';
import '../theme.css';
import { createApp } from 'vue';
import { createRouter, createWebHistory } from 'vue-router';
import App from './App.vue';
import AanvragerView from '../views/aanvrager/AanvragerView.vue';
import OrganisatieView from '../views/aanvrager/OrganisatieView.vue';

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL + 'aanvrager/'),
  routes: [
    // Inloggen en de eigen aanvragen zijn één view; die stuurt naar het adres
    // dat bij de sessie hoort.
    { path: '/', component: AanvragerView },
    { path: '/subsidieaanvragen/', component: AanvragerView },
    { path: '/organisatie', component: OrganisatieView },
  ],
});

createApp(App).use(router).mount('#app');
