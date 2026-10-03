import '@nldd/design-system';
import '@nldd/design-system/styles';
import './src/presentation/presentation.css';
import { installSheetOffset } from './src/presentation/sheetOffset.js';
import { createApp } from 'vue';
import { useColorScheme } from '@regelrecht/frontend-shared';
import App from './src/App.vue';
import router from './src/router.js';

const app = createApp(App);
// Shared colour-scheme primitive (localStorage-backed, follows the OS by
// default); the demo menu exposes the same three choices as the editor.
useColorScheme();
app.use(router);
// A left sheet next to the deck rail, not over it (temporary; see the file).
installSheetOffset(router);
app.mount('#app');
