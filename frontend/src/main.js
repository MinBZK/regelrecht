import '@nldd/design-system';
import '@nldd/design-system/styles';
import { createApp } from 'vue';
import App from './App.vue';
import router from './router.js';
import { installApiAuthGuard } from './lib/apiAuthGuard.js';

// Catch 401s on /api/* calls fired while a page is open and redirect to login,
// mirroring the router's navigation guard. Must run before any fetch.
installApiAuthGuard();

const app = createApp(App);
app.use(router);
app.mount('#app');
