import { resolve } from 'node:path';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// Drie gescheiden ingangen: publiek (landing + register), aanvrager-portaal
// en beoordelaar-portaal. Eén codebase, gedeelde backend, aparte entries.
const rewrites = [
  { match: /^\/aanvrager(\/|$)/, to: '/aanvrager.html' },
  { match: /^\/beoordelaar(\/|$)/, to: '/beoordelaar.html' },
  { match: /^\/(register(\/|$))?$/, to: '/index.html' },
];

export default defineConfig({
  // Achter het poc-portaal staat deze app onder /napp/; los draait hij op /.
  // De backend serveert zichzelf onder hetzelfde voorvoegsel (NAPP_BASE_PATH),
  // zodat het portaal het pad ongewijzigd kan doorsturen.
  base: process.env.POC_BASE ?? '/',
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
    {
      name: 'portal-rewrites',
      configureServer(server) {
        server.middlewares.use((req, _res, next) => {
          const url = req.url.split('?')[0];
          if (!url.includes('.')) {
            const hit = rewrites.find((r) => r.match.test(url));
            if (hit) req.url = hit.to;
          }
          next();
        });
      },
    },
  ],
  server: {
    port: 5400,
    fs: {
      // scenario- en wet-bestanden liggen buiten de frontend-root
      allow: ['..'],
    },
    proxy: {
      '/api': 'http://localhost:8400',
      '/auth': 'http://localhost:8400',
    },
  },
  build: {
    outDir: 'dist',
    rollupOptions: {
      input: {
        public: resolve(import.meta.dirname, 'index.html'),
        aanvrager: resolve(import.meta.dirname, 'aanvrager.html'),
        beoordelaar: resolve(import.meta.dirname, 'beoordelaar.html'),
      },
    },
  },
});
