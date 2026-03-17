import '@minbzk/storybook';
import '@minbzk/storybook/css';
import { createApp } from 'vue';
import DevApp from './DevApp.vue';

const app = createApp(DevApp);
app.mount('#dev-app');
