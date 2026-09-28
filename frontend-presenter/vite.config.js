import { fileURLToPath, URL } from 'node:url';
import path from 'node:path';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { presenterApi } from './server/presenterApi.js';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

// Decks live in ./decks unless PRESENTER_DECKS points elsewhere, so decks
// that should not end up in this public repo can stay in their own folder.
const decksRoot = path.resolve(process.env.PRESENTER_DECKS ?? here('./decks'));
// ```wet blocks look laws up here; PRESENTER_CORPUS adds more roots (colon-separated).
const corpusRoots = [
  here('../corpus/regulation'),
  here('../corpus-poc'),
  ...(process.env.PRESENTER_CORPUS ? process.env.PRESENTER_CORPUS.split(':').map((p) => path.resolve(p)) : []),
];

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
    presenterApi({ decksRoot, corpusRoots }),
  ],
  resolve: {
    // The editor's operation labels and value formatting, shared rather than
    // copied, so a wet block reads the same as the editor's Machine pane.
    alias: { '@editor-utils': here('../frontend/src/utils') },
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
