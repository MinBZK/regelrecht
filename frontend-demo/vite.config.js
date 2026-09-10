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
        // Keep echarts (only the simulation tab needs it) out of the entry
        // graph; same grouping as the editor, see frontend/vite.config.js.
        codeSplitting: {
          groups: [
            {
              name: 'echarts',
              test: /node_modules[\\/](echarts|zrender|vue-echarts)[\\/]/,
              includeDependenciesRecursively: false,
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
