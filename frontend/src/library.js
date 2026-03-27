import '@minbzk/storybook';
import '@minbzk/storybook/css';
import { createApp } from 'vue';
import App from './App.vue';
import router from './router.js';

const app = createApp(App);
app.use(router);
app.mount('#library-app');
