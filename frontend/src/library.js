import '@minbzk/storybook';
import '@minbzk/storybook/css';
import { createApp } from 'vue';
import LibraryApp from './LibraryApp.vue';

const app = createApp(LibraryApp);
app.mount('#library-app');
