import '@minbzk/storybook';
import '@minbzk/storybook/styles';
import { createApp } from 'vue';
import EditorApp from './EditorApp.vue';

const app = createApp(EditorApp);
app.mount('#editor-app');
