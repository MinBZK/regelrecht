import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// The built assets are served by `code-guide serve` from the same origin, so
// a relative base keeps `/index.html` working no matter where it is mounted.
// `server` config only matters for `npm run dev` (a standalone Vite dev server
// that proxies /api to the running Rust server); production is served by Axum.
const API_TARGET = process.env.CODE_GUIDE_API || 'http://localhost:7190';

export default defineConfig({
  base: './',
  plugins: [
    vue({
      template: {
        // The design system's components are native custom elements.
        compilerOptions: { isCustomElement: (tag) => tag.startsWith('nldd-') },
      },
    }),
  ],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    host: '0.0.0.0',
    port: 7191,
    proxy: {
      '/api': { target: API_TARGET, changeOrigin: true },
    },
  },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.js'],
  },
});
