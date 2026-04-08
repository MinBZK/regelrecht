import { createRouter, createWebHistory } from 'vue-router';
import LawEntriesView from './views/LawEntriesView.vue';
import JobsView from './views/JobsView.vue';

export default createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/law-entries' },
    { path: '/law-entries', name: 'law-entries', component: LawEntriesView },
    { path: '/jobs', name: 'jobs', component: JobsView },
  ],
});
