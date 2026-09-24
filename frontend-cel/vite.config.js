import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// De runtime draait los (`just cel`); deze dev-server proxyt /api,
// /cellen en /processen naar hem. Beide poorten liggen in 7100-7300, zodat ze ook vanuit
// een dev-container bereikbaar zijn.
const runtime = `http://127.0.0.1:${process.env.CEL_PORT ?? '7170'}`;

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
  server: {
    host: '0.0.0.0',
    port: Number(process.env.CEL_FRONTEND_PORT ?? 7171),
    strictPort: true,
    proxy: {
      '/api': runtime,
      '/cellen': runtime,
      '/processen': runtime,
    },
  },
});
