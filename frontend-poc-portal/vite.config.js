import { defineConfig } from 'vite';

// The portal renders its two pages in Rust, so this build produces no HTML —
// only the design-system bundle those pages link to. The filenames are
// therefore fixed rather than hashed: `pagina.rs` writes
// `/_assets/nldd.{css,js}` as literals, and a hash would break that link on
// every dependency bump.
//
// Cache-busting is not lost, only moved: the portal serves `/_assets` with the
// short-lived caching a rarely-changing, unhashed asset wants (see
// packages/poc-portal/src/app.rs).
export default defineConfig({
  // The bundle is served from /_assets/, and the design system's CSS points at
  // its own font files. Without this those URLs come out root-absolute
  // (`url(/RijksSansWeb-Regular.woff2)`) and every font 404s — the page then
  // renders in a fallback face, and the browser waits on four requests that
  // never arrive.
  base: '/_assets/',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // No module preload polyfill: the bundle is one entry, loaded as a plain
    // <script type="module">, and the polyfill would add an inline script the
    // CSP deliberately does not allow.
    modulePreload: false,
    rollupOptions: {
      input: 'nldd-components.js',
      output: {
        entryFileNames: 'nldd.js',
        assetFileNames: (info) =>
          info.names?.some((n) => n.endsWith('.css')) ? 'nldd.css' : '[name][extname]',
      },
    },
  },
});
