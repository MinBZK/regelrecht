import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// De cel draait los (`just cel`); deze dev-server proxyt /api naar
// hem. Beide poorten liggen in 7100-7300, zodat ze ook vanuit een
// dev-container bereikbaar zijn.
const celPoort = process.env.AANVRAAG_CEL_PORT ?? '7170';

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
    port: Number(process.env.AANVRAAG_CEL_FRONTEND_PORT ?? 7171),
    strictPort: true,
    proxy: {
      '/api': `http://127.0.0.1:${celPoort}`,
    },
  },
});
