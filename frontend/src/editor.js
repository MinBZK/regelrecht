import '@minbzk/storybook';
import '@minbzk/storybook/styles';
import { createApp } from 'vue';
import EditorApp from './EditorApp.vue';

async function init() {
  try {
    const res = await fetch('/auth/status');
    if (res.ok) {
      const status = await res.json();
      if (status.oidc_configured && !status.authenticated) {
        window.location.href = '/auth/login';
        return;
      }
    }
  } catch {
    // Auth endpoint not available — proceed without auth
  }

  const app = createApp(EditorApp);
  app.mount('#editor-app');
}

init();
