import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { resolve } from 'path';

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
      name: 'library-spa-fallback',
      configureServer(server) {
        server.middlewares.use((req, _res, next) => {
          const url = req.url.split('?')[0];
          if (url === '/' || (url.startsWith('/library') && !url.includes('.'))) {
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
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        editor: resolve(__dirname, 'editor.html'),
      },
    },
  },
  server: {
    port: 3000,
    watch: {
      usePolling: true,
      interval: 1000,
    },
    proxy: {
      '/api': 'http://localhost:8000',
      '/auth': 'http://localhost:8000',
      '/health': 'http://localhost:8000',
    },
  },
});
