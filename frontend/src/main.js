import '@nldd/design-system';
import '@nldd/design-system/styles';
import { createApp } from 'vue';
import App from './App.vue';
import router from './router.js';
import { installApiAuthGuard } from './lib/apiAuthGuard.js';
import { installDevSlowMode } from './lib/devSlowMode.js';

// Catch 401s on /api/* calls fired while a page is open and redirect to login,
// mirroring the router's navigation guard. Must run before any fetch.
installApiAuthGuard();

// Dev-only: `?slow=1500` delays /api/* responses so loading states are actually
// visible locally. No-op in a production build and when unset. Installed after
// the auth guard so a 401 redirect is not held up by the delay.
installDevSlowMode();

const app = createApp(App);
app.use(router);
app.mount('#app');
