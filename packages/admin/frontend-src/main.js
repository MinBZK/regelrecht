import '@nldd/design-system';
import '@nldd/design-system/styles';

import { createApp } from 'vue';
import App from './src/App.vue';
import router from './src/router.js';

const app = createApp(App);
app.use(router);
app.mount('#admin-app');
