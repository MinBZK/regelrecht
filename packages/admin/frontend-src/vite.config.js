import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  root: '.',
  // Vite's built-in SPA history-fallback (the default for appType 'spa')
  // already serves index.html for client-side routes while correctly
  // excluding vite internals (/@vite/client, /@id/, /node_modules, …) and
  // requests with a file extension. A previous hand-rolled
  // `admin-spa-fallback` plugin rewrote *every* extensionless request —
  // including /@vite/client — to /index.html, which breaks module loading
  // under Vite 8 (blank screen in `vite dev`). Rely on the built-in instead.
  appType: 'spa',
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
  ],
  build: {
    cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
    outDir: '../static',
    emptyOutDir: true,
  },
  server: {
    port: 3001,
    proxy: {
      '/api': 'http://localhost:8001',
      '/auth': 'http://localhost:8001',
      '/health': 'http://localhost:8001',
    },
  },
});
