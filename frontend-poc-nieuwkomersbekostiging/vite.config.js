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
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.js', 'tests/**/*.test.js'],
    testTimeout: 20000,
    server: {
      deps: {
        inline: [/@cucumber\//],
      },
    },
  },
  server: {
    port: 3200,
    proxy: {
      // Beleidsassistent-backend (server/); just server start hem op 3700.
      '/api': 'http://localhost:3700',
    },
  },
});
