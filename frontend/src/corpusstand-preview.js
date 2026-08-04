// Entry point voor de geïsoleerde Corpusstand-preview (dev/demo, geen backend).
import '@nldd/design-system';
import '@nldd/design-system/styles';
import { createApp } from 'vue';
import CorpusstandPreview from './corpusstand-preview/CorpusstandPreview.vue';

createApp(CorpusstandPreview).mount('#app');
