import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { resolve } from 'path';

export default defineConfig({
  root: '.',
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('rr-'),
        },
      },
    }),
  ],
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
  },
});
