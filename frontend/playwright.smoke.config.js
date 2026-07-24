import { defineConfig } from '@playwright/test';
import { resolve } from 'node:path';

// Isolated Playwright project for the deployed-preview smoke test. Unlike the
// mocked `e2e/` suite (own Vite on :7100, intercepted backend), this suite runs
// the real editor against a DEPLOYED preview (or production): a real Keycloak
// login, the real editor-api + DB + WASM. Hence: no `webServer`, `baseURL`
// comes from the environment, and a `globalSetup` logs in once and hands every
// spec a persisted authenticated session.
//
// Run it via `just smoke-preview PR=<n>` (or `URL=<full-url>`), which sets
// SMOKE_BASE_URL and reads the test-user creds from /workspace/.cred into
// SMOKE_USER / SMOKE_PASS.

const STORAGE_STATE = resolve(import.meta.dirname, '.auth', 'state.json');

export default defineConfig({
  testDir: './e2e-smoke',
  // Traject writes commit + push to a git remote, so a save round-trip can take
  // a minute. Budget generously per test; the individual waits are bounded too.
  timeout: 240_000,
  expect: { timeout: 15_000 },
  // A deployed preview is a shared, sometimes-cold environment; one retry
  // absorbs a transient blip without masking a real regression.
  retries: 1,
  // The flow is stateful (create -> edit -> persist -> teardown), so it must run
  // in order on a single worker.
  workers: 1,
  fullyParallel: false,
  reporter: [['list']],
  globalSetup: './e2e-smoke/global-setup.js',
  use: {
    baseURL: process.env.SMOKE_BASE_URL,
    storageState: STORAGE_STATE,
    headless: true,
    // A cold preview can be slow to boot the SPA and its API.
    navigationTimeout: 60_000,
    actionTimeout: 30_000,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    // Wide enough that the editor renders its panes side-by-side (the Machine
    // pane is visible without a pane-switch).
    viewport: { width: 1600, height: 1000 },
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' },
    },
  ],
});
