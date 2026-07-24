import { chromium } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { login } from './smoke-helpers.js';

// Where the authenticated session is stored. Kept next to the config (frontend/
// .auth/state.json) and git-ignored - it holds the Keycloak session cookie.
export const STORAGE_STATE = resolve(import.meta.dirname, '..', '.auth', 'state.json');

/**
 * Global setup for the deployed-preview smoke suite: log in once with the
 * shared test user, verify the session, and persist `storageState` so every
 * spec reuses the same authenticated context without re-driving Keycloak.
 *
 * All environment-specific input comes from env vars (never hardcoded):
 *   SMOKE_BASE_URL  - the preview/prod base URL (also set as `use.baseURL`)
 *   SMOKE_USER      - Keycloak username of the shared test user
 *   SMOKE_PASS      - its password
 */
export default async function globalSetup(config) {
  const baseURL =
    process.env.SMOKE_BASE_URL || config.projects?.[0]?.use?.baseURL;
  const user = process.env.SMOKE_USER;
  const pass = process.env.SMOKE_PASS;

  if (!baseURL) {
    throw new Error('SMOKE_BASE_URL is required (the deployed preview/prod base URL).');
  }
  if (!user || !pass) {
    throw new Error(
      'SMOKE_USER and SMOKE_PASS are required. The `just smoke-preview` recipe injects them from the local cred file.',
    );
  }

  const browser = await chromium.launch();
  try {
    const context = await browser.newContext({ baseURL });
    const page = await context.newPage();

    await login(page, { user, pass });

    mkdirSync(dirname(STORAGE_STATE), { recursive: true });
    await context.storageState({ path: STORAGE_STATE });
    await context.close();
  } finally {
    await browser.close();
  }
}
