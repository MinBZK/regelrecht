import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// The runtime runs separately (`just cel`); this dev server proxies /api,
// /cells and /processes to it. Both ports are in 7100-7300, so they can also
// be reached from a dev container.
const runtime = `http://127.0.0.1:${process.env.CELL_PORT ?? '7170'}`;

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
  ],
  test: {
    include: ['src/**/*.test.js'],
  },
  server: {
    host: '0.0.0.0',
    port: Number(process.env.CELL_FRONTEND_PORT ?? 7171),
    strictPort: true,
    proxy: {
      '/api': runtime,
      '/cells': runtime,
      '/processes': runtime,
    },
  },
});
