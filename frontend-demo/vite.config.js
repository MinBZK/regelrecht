import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// Port ligt binnen het Docker-mapping bereik (7100-7300) dat de dev-omgeving
// vanuit Windows bereikbaar maakt, en bindt op 0.0.0.0 zodat host-access werkt.
export default defineConfig({
  root: '.',
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('ndd-'),
        },
      },
    }),
    {
      name: 'spa-fallback',
      configureServer(server) {
        server.middlewares.use((req, _res, next) => {
          const url = req.url.split('?')[0];
          if (!url.includes('.') && !url.startsWith('/wasm') && !url.startsWith('/demo-assets')) {
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
  },
  build: {
    outDir: 'dist',
  },
  server: {
    host: '0.0.0.0',
    port: 7180,
    watch: {
      usePolling: true,
      interval: 1000,
    },
    proxy: {
      '/api': 'http://localhost:7181',
    },
  },
});
