import { existsSync } from 'node:fs';
import { createServer } from 'node:net';
import { resolve } from 'node:path';

import { defineConfig } from '@playwright/test';

// De browserbewijsronde van de chronolexografie-testopstelling: dezelfde
// checks die met de hand door de publieke wereld liepen, nu als suite die bij
// elke PR draait.
//
// Standaard start deze configuratie de opstelling zelf: de server uit
// packages/chrono-poc-web op een vrije poort, met de publieke wereld en zonder
// login (geen OIDC_CLIENT_ID = elke route open, de lokale modus van die crate).
// Draai hem via `just chrono-poc-e2e`; dat recept bouwt eerst de bundel en de
// binary, zodat het opstarten hieronder een kwestie van seconden is.
//
// Drie omgevingsvariabelen verzetten het doel, zonder een tweede suite:
//
//   E2E_WORLD=<pad>   een ander wereldbestand (relatief aan de repo-root of
//                     absoluut). De checks die over de casus van de publieke
//                     wereld gaan horen dan niet te slagen; wat meereist is de
//                     vorm van de opstelling.
//   E2E_BASE=<url>    een opstelling die al draait — een deployment, of een
//                     server die je zelf startte. Er wordt dan niets gestart.
//   E2E_COOKIE=<a=b>  de sessiecookie voor zo'n deployment (achter de login).

const repoRoot = resolve(import.meta.dirname, '..');
const staticDir = resolve(repoRoot, 'frontend-chrono-poc', 'dist');
const world = resolve(
  repoRoot,
  process.env.E2E_WORLD || 'packages/simulator/worlds/publieke_wereld.yaml',
);

/**
 * Een vrije poort, bij voorkeur uit 7180-7300.
 *
 * 7100-7300 is wat de dev-container naar de host doorzet, zodat een blijvende
 * server (`--debug`, `--ui`) ook buiten de container te bekijken is. Het telt
 * pas vanaf 7180 zodat de poorten die een mens zelf aanwijst — `just
 * chrono-poc` staat op 7160 — niet onder zijn handen vandaan gegrepen worden.
 * Is alles bezet, meerdere worktrees naast elkaar, dan doet elke vrije poort
 * het ook.
 */
async function freePort() {
  const available = (port) =>
    new Promise((accept) => {
      const probe = createServer();
      probe.on('error', () => accept(0));
      probe.listen(port, '0.0.0.0', () => {
        const chosen = probe.address().port;
        probe.close(() => accept(chosen));
      });
    });

  for (let port = 7180; port <= 7300; port += 1) {
    const chosen = await available(port);
    if (chosen) return chosen;
  }
  return await available(0);
}

// De poort wordt één keer gekozen, in het hoofdproces. Elke worker laadt dit
// bestand opnieuw; zonder deze doorgifte zou zo'n worker een tweede poort
// kiezen waar niets op luistert. `e2e/world.js` leest dezelfde variabele.
const baseURL =
  process.env.CHRONO_POC_E2E_BASE || process.env.E2E_BASE || `http://localhost:${await freePort()}`;
process.env.CHRONO_POC_E2E_BASE = baseURL;

const ownServer = !process.env.E2E_BASE;

if (ownServer && !existsSync(resolve(staticDir, 'index.html'))) {
  throw new Error(
    `Geen gebouwde frontend in ${staticDir}. Draai \`just chrono-poc-e2e\` (die bouwt hem) ` +
      'of `just build-chrono-poc`.',
  );
}

/**
 * De sessiecookie voor een opstelling achter de login, als `naam=waarde`.
 *
 * Alleen zinvol samen met E2E_BASE: bij een eigen server staat de login uit.
 */
function cookieState() {
  if (!process.env.E2E_COOKIE) return undefined;
  const url = new URL(baseURL);
  const cookies = process.env.E2E_COOKIE.split(';')
    .map((pair) => pair.trim())
    .filter(Boolean)
    .map((pair) => {
      const at = pair.indexOf('=');
      return {
        name: pair.slice(0, at),
        value: pair.slice(at + 1),
        domain: url.hostname,
        path: '/',
        expires: -1,
        httpOnly: true,
        secure: url.protocol === 'https:',
        sameSite: 'Lax',
      };
    });
  return { cookies, origins: [] };
}

// De sandbox van Chromium leunt op user-namespaces, en die staan in de
// dev-container uit; daar start de browser alleen met --no-sandbox. Op een
// GitHub-runner is de sandbox er wel en blijft hij aan — hem daar uitzetten zou
// beschermingslagen weghalen die niets kosten.
const noSandbox = process.env.E2E_NO_SANDBOX === '1' || (!process.env.CI && existsSync('/.dockerenv'));

export default defineConfig({
  testDir: './e2e',
  testMatch: '**/*.spec.js',
  timeout: 60_000,
  expect: { timeout: 15_000 },
  // De suite is één verhaal door de wereld: elke stap bouwt op de vorige, en de
  // wereld hangt aan één sessie. Parallel draaien zou dat verhaal in stukken
  // knippen die elkaars stand missen.
  workers: 1,
  fullyParallel: false,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : [['list']],
  outputDir: 'test-results',
  use: {
    baseURL,
    headless: true,
    storageState: cookieState(),
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    launchOptions: noSandbox ? { args: ['--no-sandbox', '--disable-dev-shm-usage'] } : {},
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
  webServer: ownServer
    ? {
        // `cargo run` en niet het pad naar de binary: het recept heeft hem al
        // gebouwd, dus dit is een start en geen build — maar wie de suite los
        // draait krijgt hem alsnog gebouwd in plaats van een foutmelding.
        command: 'cargo run --quiet -p regelrecht-chrono-poc-web --bin chrono-poc-web',
        cwd: resolve(repoRoot, 'packages'),
        url: `${baseURL}/health`,
        // Ruim genoeg voor een koude cargo-build; is er al gebouwd, dan is de
        // server binnen een paar seconden op.
        timeout: 600_000,
        reuseExistingServer: false,
        stdout: 'pipe',
        stderr: 'pipe',
        env: {
          CHRONO_POC_PORT: String(new URL(baseURL).port),
          CHRONO_POC_WORLD_SOURCE: `local:${world}`,
          STATIC_DIR: staticDir,
        },
      }
    : undefined,
});
