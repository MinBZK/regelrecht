import { defineConfig } from 'vite';
import { resolve } from 'path';
import vue from '@vitejs/plugin-vue';

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
    outDir: 'dist',
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        'editor-example': resolve(__dirname, 'editor-example.html'),
        editor: resolve(__dirname, 'editor.html'),
      },
    },
  },
  server: {
    port: 3000,
  },
});
