import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  // Achter het poc-portaal staat deze app onder /<slug>/; los draait hij op /.
  // De Docker-build zet POC_BASE, `npm run dev` laat 'm leeg.
  //
  // Dit herschrijft wat de bundler zelf uitgeeft. Wat wij met de hand in een
  // fetch() zetten gaat langs src/basePad.js, dat dezelfde waarde leest.
  base: process.env.POC_BASE ?? '/',
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
  ],
  // De testconfiguratie staat in vitest.config.js: vitest 4 leest een
  // `test`-blok hier niet meer.
  server: {
    port: 3100,
    proxy: {
      // Beleidsassistent-backend (server/); just server start hem op 3600.
      '/api': 'http://localhost:3600',
    },
  },
});
