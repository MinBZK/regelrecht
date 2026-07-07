import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// @cucumber/messages 33 runs `createRequire(import.meta.url)('../package.json')`
// at import time (a Node-only API). In the browser build that throws and
// crashes every view importing the gherkin parser, so alias `node:module` to a
// browser shim that provides a harmless `createRequire`. Scope it to the build
// only — under vitest (Node) the real `node:module` works, so we leave it.
const isVitest = !!process.env.VITEST;
const nodeModuleShim = fileURLToPath(
  new URL('./src/shims/node-module.js', import.meta.url),
);

// Backend port the dev proxy forwards /api, /auth and /health to. Defaults to
// 8000 (editor-api); `just dev-frontend` sets API_PORT so multiple backends can
// coexist on distinct ports.
const apiTarget = `http://localhost:${process.env.API_PORT || '8000'}`;

export default defineConfig({
  root: '.',
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
    {
      name: 'spa-fallback',
      configureServer(server) {
        server.middlewares.use((req, _res, next) => {
          const url = req.url.split('?')[0];
          if (
            url === '/' ||
            url === '/editor.html' ||
            (url.startsWith('/library') && !url.includes('.')) ||
            (url.startsWith('/editor') && !url.includes('.'))
          ) {
            req.url = '/index.html';
          }
          next();
        });
      },
    },
  ],
  resolve: {
    alias: isVitest ? {} : { 'node:module': nodeModuleShim },
  },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.js'],
    pool: 'vmThreads',
    testTimeout: 10000,
    server: {
      // @cucumber/gherkin 40 and @cucumber/messages 33 ship as pure ESM. The
      // vmThreads pool loads external ESM in a separate VM context, which throws
      // "Linked modules must use the same context". Inlining lets vitest process
      // them in the test context instead. The @regelrecht/frontend-shared
      // workspace package (ESM) hits the same issue when a test transitively
      // imports it (e.g. usePollingFetch → apiFetch), so inline it too.
      // echarts/vue-echarts/zrender are also pure ESM (imported by the
      // harvester dashboard charts).
      deps: {
        inline: [/@cucumber\//, /@regelrecht\//, /^echarts/, /^vue-echarts/, /^zrender/],
      },
    },
  },
  build: {
    cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
    outDir: 'dist',
  },
  server: {
    port: 3000,
    proxy: {
      '/api': apiTarget,
      '/auth': apiTarget,
      '/health': apiTarget,
    },
  },
});
