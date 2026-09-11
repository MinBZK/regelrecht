import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// De server die de wereld-API en /health aanbiedt. 8000 is de poort waarop de
// PoC-server draait; API_PORT verzet hem zodat meerdere backends naast elkaar
// kunnen leven (zelfde afspraak als frontend/vite.config.js).
const apiTarget = `http://localhost:${process.env.API_PORT || '8000'}`;

// Browser-poort van de dev-server. Binnen 7100-7300 en op 0.0.0.0, zodat de
// pagina ook buiten de container bereikbaar is. VITE_PORT verzet hem.
const port = Number(process.env.VITE_PORT || 7250);

export default defineConfig({
  root: '.',
  plugins: [
    vue({
      template: {
        compilerOptions: {
          // Elke nldd-* tag is een web component van het ontwerpsysteem, geen
          // Vue-component.
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
  ],
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.js'],
    server: {
      // @regelrecht/frontend-shared is een pure-ESM workspace-package; vitest
      // moet hem zelf verwerken in plaats van als externe module te laden.
      deps: {
        inline: [/@regelrecht\//],
      },
    },
  },
  build: {
    cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
    outDir: 'dist',
  },
  server: {
    port,
    strictPort: true,
    host: '0.0.0.0',
    proxy: {
      '/api': { target: apiTarget, changeOrigin: true },
      '/health': { target: apiTarget, changeOrigin: true },
    },
  },
});
