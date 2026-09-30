import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// @cucumber/messages runs `createRequire(import.meta.url)` at import time, a
// Node-only API. The shared Gherkin runner pulls it in, so the browser build
// aliases `node:module` to a harmless shim (same trick as the editor). Under
// vitest the real module works, so the alias is skipped there.
const isVitest = !!process.env.VITEST;
const nodeModuleShim = fileURLToPath(new URL('./src/shims/node-module.js', import.meta.url));

export default defineConfig({
  root: '.',
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
  ],
  resolve: {
    alias: isVitest ? {} : { 'node:module': nodeModuleShim },
  },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.js'],
    server: {
      deps: {
        inline: [/@cucumber\//, /@regelrecht\//],
      },
    },
  },
  build: {
    cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
    outDir: 'dist',
    rolldownOptions: {
      output: {
        // Group echarts into its own chunk, as the editor does (see
        // frontend/vite.config.js).
        //
        // Deliberately WITHOUT the editor's `includeDependenciesRecursively:
        // false`. That option keeps the chunk out of the entry graph entirely,
        // but here it also leaves a shared helper behind that the chunk still
        // calls, and every chart then dies on `TypeError: <helper> is not a
        // function`. Vue's async-component boundary swallows that error, so the
        // build succeeds, the page loads, and only the charts stay blank — which
        // is exactly how it reached production unnoticed. The editor gets away
        // with the option because it imports echarts straight from
        // node_modules; here it arrives through SimBarChart.vue, whose helpers
        // the dependency walk would have to carry along.
        //
        // The cost is that echarts is modulepreloaded on first paint (~570 kB
        // over the entry). A working simulation is worth more than that; if the
        // first load has to come down, the fix is to make the chart component
        // reachable only through its own async chunk, not to switch this option
        // back on.
        codeSplitting: {
          groups: [
            {
              name: 'echarts',
              test: /node_modules[\\/](echarts|zrender|vue-echarts)[\\/]/,
            },
          ],
        },
      },
    },
  },
  server: {
    port: 3000,
  },
});
