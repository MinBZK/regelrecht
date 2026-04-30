import '@minbzk/storybook';
import '@minbzk/storybook/styles';
import { createApp } from 'vue';
import App from './App.vue';
import router from './router.js';
import { useUserSettings } from './composables/useUserSettings.js';

// Side-effect: starts the /api/user/settings fetch and registers a
// watchEffect that mirrors the theme onto <html data-scheme="...">.
useUserSettings();

const app = createApp(App);
app.use(router);
app.mount('#app');
