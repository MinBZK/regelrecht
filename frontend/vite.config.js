import { readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
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
      // Serve the graph builder's payloads under /corpusgraaf during `vite
      // dev`, from wherever the builder dropped them. Without this the two
      // "Echt corpus" options on /graph3d.html get the HTML fallback instead of
      // a payload, and the demo page can only ever show synthetic graphs - so
      // every judgement about the real corpus would be made on made-up data.
      // Dev only, and it reads nothing but `.rrgraph`-shaped filenames.
      name: 'corpusgraaf-payloads',
      apply: 'serve',
      configureServer(server) {
        server.middlewares.use('/corpusgraaf', async (req, res, next) => {
          const name = req.url.split('?')[0].replace(/^\//, '');
          if (!/^[\w.-]+\.rrgraph$/.test(name)) return next();
          try {
            const path = join(process.env.CORPUSGRAAF_DIR || '/tmp/corpusgraaf', name);
            const info = await stat(path);
            const body = await readFile(path);
            res.setHeader('content-type', 'application/octet-stream');
            res.setHeader('content-length', info.size);
            res.end(body);
          } catch {
            res.statusCode = 404;
            res.end('geen payload');
          }
        });
      },
    },
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
    rollupOptions: {
      // Vite only builds index.html by default; the 3D-graph pages are extra
      // entries, so they exist in a production build too instead of only under
      // `vite dev`.
      input: {
        index: fileURLToPath(new URL('./index.html', import.meta.url)),
        graph3d: fileURLToPath(new URL('./graph3d.html', import.meta.url)),
        benchGraph3d: fileURLToPath(new URL('./bench-graph3d.html', import.meta.url)),
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
