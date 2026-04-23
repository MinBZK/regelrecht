import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

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
  },
  build: {
    cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
    outDir: 'dist',
  },
  server: {
    port: 3000,
    proxy: {
      '/api': 'http://localhost:8000',
      '/auth': 'http://localhost:8000',
      '/health': 'http://localhost:8000',
    },
  },
});
