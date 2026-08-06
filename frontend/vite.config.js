import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// @cucumber/messages 34 runs `createRequire(import.meta.url)('../package.json')`
// at import time (a Node-only API). In the browser build that throws and
// crashes every view importing the gherkin parser, so alias `node:module` to a
// browser shim that provides a harmless `createRequire`. Scope it to the build
// only - under vitest (Node) the real `node:module` works, so we leave it.
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
      // @cucumber/gherkin 41 and @cucumber/messages 34 ship as pure ESM. The
      // vmThreads pool loads external ESM in a separate VM context, which throws
      // "Linked modules must use the same context". Inlining lets vitest process
      // them in the test context instead. The @regelrecht/frontend-shared
      // workspace package (ESM) hits the same issue when a test transitively
      // imports it (e.g. usePollingFetch → apiFetch), so inline it too.
      deps: {
        inline: [/@cucumber\//, /@regelrecht\//],
      },
    },
  },
  build: {
    cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
    outDir: 'dist',
    rolldownOptions: {
      output: {
        // Without this group, rolldown parks its shared module-namespace
        // runtime helper inside the OverviewView chunk (the only chunk that
        // needs CJS interop, via echarts). Several design-system chunks in the
        // static entry graph import that helper, which drags the ~550 KB
        // echarts payload into index.html's modulepreload list even though the
        // route itself is lazy. Grouping echarts into its own chunk makes
        // rolldown emit a separate tiny runtime chunk, and echarts is then
        // only fetched when a harvester view actually opens.
        //
        // `includeDependenciesRecursively: false` is required: the default
        // (true) pulls the captured modules' dependencies (Vue's runtime-core,
        // reactivity, tslib) into the echarts chunk, which puts it right back
        // in the entry graph.
        advancedChunks: {
          groups: [
            {
              name: 'echarts',
              test: /node_modules[\\/](echarts|zrender|vue-echarts)[\\/]/,
              includeDependenciesRecursively: false,
            },
          ],
        },
      },
    },
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
