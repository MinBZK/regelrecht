import { fileURLToPath, URL } from 'node:url';
import path from 'node:path';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { presenterApi } from './server/presenterApi.js';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const isVitest = !!process.env.VITEST;

// Decks live in ./decks unless PRESENTER_DECKS points elsewhere, so decks
// that should not end up in this public repo can stay in their own folder.
const decksRoot = path.resolve(process.env.PRESENTER_DECKS ?? here('./decks'));
// ```wet blocks look laws up here; PRESENTER_CORPUS adds more roots (colon-separated).
const corpusRoots = [
  here('../corpus/regulation'),
  here('../corpus-poc'),
  ...(process.env.PRESENTER_CORPUS ? process.env.PRESENTER_CORPUS.split(':').map((p) => path.resolve(p)) : []),
];
// The engine for ```reken blocks: the output of `just wasm-build`.
const wasmDir = path.resolve(process.env.PRESENTER_WASM ?? here('../frontend/public/wasm/pkg'));

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
    presenterApi({ decksRoot, corpusRoots, wasmDir }),
  ],
  resolve: {
    // The editor's operation labels and value formatting, shared rather than
    // copied, so a wet block reads the same as the editor's Machine pane.
    // outputFormat.js imports @regelrecht/frontend-shared; the reken block
    // imports its Gherkin runner directly.
    alias: {
      '@editor-utils': here('../frontend/src/utils'),
      // The Gherkin parser behind ```reken blocks pulls in @cucumber/messages,
      // which calls Node's createRequire at import time; the browser gets the
      // same shim as the editor and the demo. Vitest runs in Node and keeps
      // the real module.
      ...(isVitest ? {} : { 'node:module': here('./src/shims/node-module.js') }),
    },
  },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.js', 'server/**/*.test.js'],
    server: { deps: { inline: [/@regelrecht\//] } },
  },
  server: {
    port: 5180,
    fs: { allow: [here('..')] },
  },
});
