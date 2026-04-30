// Intercept ?test-sso before Vue loads. The router redirects / → /law-entries
// which drops query params, so we must check here before anything else runs.
if (new URLSearchParams(window.location.search).has('test-sso')) {
  window.location.replace('/auth/test-login');
} else {
  import('@minbzk/storybook');
  import('@minbzk/storybook/styles');

  const { createApp } = await import('vue');
  const { default: App } = await import('./src/App.vue');
  const { default: router } = await import('./src/router.js');

  const app = createApp(App);
  app.use(router);
  app.mount('#admin-app');
}
