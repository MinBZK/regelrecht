import '@nldd/design-system';
import '@nldd/design-system/styles';
import '../theme.css';
import { createApp } from 'vue';
import { createRouter, createWebHistory } from 'vue-router';
import App from './App.vue';
import AanvragerView from '../views/aanvrager/AanvragerView.vue';
import AanvraagNieuwView from '../views/aanvrager/AanvraagNieuwView.vue';
import AanvraagStatusView from '../views/aanvrager/AanvraagStatusView.vue';
import OrganisatieView from '../views/aanvrager/OrganisatieView.vue';

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL + 'aanvrager/'),
  routes: [
    { path: '/', component: AanvragerView },
    { path: '/nieuw', component: AanvraagNieuwView },
    { path: '/aanvraag/:id', component: AanvraagStatusView },
    { path: '/organisatie', component: OrganisatieView },
  ],
});

createApp(App).use(router).mount('#app');
