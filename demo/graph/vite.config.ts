import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: {
    proxy: {
      '/laws': 'http://127.0.0.1:8000',
      '/law': 'http://127.0.0.1:8000',
    },
  },
});
