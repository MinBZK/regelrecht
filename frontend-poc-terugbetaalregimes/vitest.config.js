import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';

// Een eigen bestand, niet het `test`-blok in vite.config.js: vitest 4 leest dat
// blok niet meer. Dat is geen cosmetisch verschil — `testTimeout` viel stil
// terug op de standaard, en de populatietest (32 seconden echt werk) ging
// daardoor om met een timeout terwijl dezelfde test op vitest 2 gewoon slaagde.
// Een genegeerde instelling die eruitziet als een instelling is precies het
// soort fout dat pas opvalt als iemand de suite draait.
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
    environment: 'happy-dom',
    include: ['src/**/*.test.js', 'tests/**/*.test.js'],
    // De populatiesimulatie draait de echte WASM-engine over duizenden
    // records; 32 seconden is normaal, geen symptoom.
    testTimeout: 60000,
    server: {
      deps: {
        inline: [/@cucumber\//],
      },
    },
  },
});
