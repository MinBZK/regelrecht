import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// The built assets are served by `arch-extract serve` from the same origin, so
// a relative base keeps `/index.html` working no matter where it is mounted.
// `server` config only matters for `npm run dev` (a standalone Vite dev server
// that proxies /api to the running Rust server) — production is served by Axum.
const API_TARGET = process.env.ARCH_EXPLORE_API || 'http://localhost:7180';

export default defineConfig({
  base: './',
  plugins: [vue()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    host: '0.0.0.0',
    port: 7181,
    proxy: {
      '/api': { target: API_TARGET, changeOrigin: true },
    },
  },
});
