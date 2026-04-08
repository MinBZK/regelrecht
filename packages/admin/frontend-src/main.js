import '@minbzk/storybook';
import '@minbzk/storybook/styles';

import { createApp } from 'vue';
import App from './src/App.vue';
import router from './src/router.js';

const app = createApp(App);
app.use(router);
app.mount('#admin-app');
