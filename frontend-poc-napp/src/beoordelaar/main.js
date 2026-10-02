import '@nldd/design-system';
import '@nldd/design-system/styles';
import '../theme.css';
import { createApp } from 'vue';
import { createRouter, createWebHistory } from 'vue-router';
import App from './App.vue';
import BeoordelaarView from '../views/beoordelaar/BeoordelaarView.vue';
import BeoordelingView from '../views/beoordelaar/BeoordelingView.vue';
import PartijregisterView from '../views/beoordelaar/PartijregisterView.vue';
import PartijDetailView from '../views/beoordelaar/PartijDetailView.vue';
import ScenarioRunnerView from '../views/beoordelaar/ScenarioRunnerView.vue';

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL + 'beoordelaar/'),
  routes: [
    { path: '/', component: BeoordelaarView },
    { path: '/aanvraag/:id', component: BeoordelingView },
    { path: '/partijregister', component: PartijregisterView },
    { path: '/partijregister/:kvk', component: PartijDetailView },
    { path: '/scenarios', component: ScenarioRunnerView },
  ],
});

createApp(App).use(router).mount('#app');
