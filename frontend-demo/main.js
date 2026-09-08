import '@nldd/design-system';
import '@nldd/design-system/styles';
import { createApp } from 'vue';
import { useColorScheme } from '@regelrecht/frontend-shared';
import App from './src/App.vue';
import router from './src/router.js';

const app = createApp(App);
// Shared colour-scheme primitive (localStorage-backed, follows the OS by
// default); the demo menu exposes the same three choices as the editor.
useColorScheme();
app.use(router);
app.mount('#app');
