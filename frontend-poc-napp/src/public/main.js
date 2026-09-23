import '@nldd/design-system';
import '@nldd/design-system/styles';
import '../theme.css';
import { createApp } from 'vue';
import { createRouter, createWebHistory } from 'vue-router';
import App from './App.vue';
import HomeView from '../views/HomeView.vue';
import RegisterView from '../views/RegisterView.vue';

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL + ''),
  routes: [
    { path: '/', component: HomeView },
    { path: '/register', component: RegisterView },
  ],
});

createApp(App).use(router).mount('#app');
