import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

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
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.js'],
    pool: 'vmThreads',
    testTimeout: 10000,
    server: {
      // @cucumber/gherkin 40 and @cucumber/messages 33 ship as pure ESM.
      // The vmThreads pool loads external ESM in a separate VM context, which
      // throws "Linked modules must use the same context". Inlining lets vitest
      // process them in the test context instead.
      deps: {
        inline: [/@cucumber\//],
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
