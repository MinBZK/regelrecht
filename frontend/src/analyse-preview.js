// Entry point voor de geïsoleerde Analyse-preview (dev/demo, geen backend).
import '@nldd/design-system';
import '@nldd/design-system/styles';
import { createApp } from 'vue';
import AnalysePreview from './analyse-preview/AnalysePreview.vue';

createApp(AnalysePreview).mount('#app');
