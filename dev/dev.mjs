#!/usr/bin/env node
// Cross-platform dev process manager
// Replaces the bash-only `just dev` recipe so it works on Windows + Linux

import { execSync, spawn } from "node:child_process";
import { existsSync, writeFileSync, openSync, closeSync } from "node:fs";
import { resolve, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { platform } from "node:os";

const isWindows = platform() === "win32";
const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");

// --- ANSI colors ---
const bold = "\x1b[1m";
const dim = "\x1b[2m";
const reset = "\x1b[0m";
const green = "\x1b[32m";
const red = "\x1b[31m";
const yellow = "\x1b[33m";

// --- Helpers ---

function log(msg) {
  process.stdout.write(msg);
}

function env(key, fallback) {
  return process.env[key] || fallback;
}

/** Check if a command exists on PATH */
function hasCommand(name) {
  try {
    const cmd = isWindows ? `where ${name}` : `command -v ${name}`;
    execSync(cmd, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

/** Run a shell command synchronously, return true on success */
function run(cmd, opts = {}) {
  try {
    execSync(cmd, { stdio: "ignore", cwd: root, ...opts });
    return true;
  } catch {
    return false;
  }
}

/** Run a shell command and return stdout */
function runOutput(cmd, opts = {}) {
  return execSync(cmd, { cwd: root, encoding: "utf8", ...opts }).trim();
}

const compose = "docker compose -f docker-compose.dev.yml -f dev/compose.native.yaml";

// --- Preflight checks ---

log(`${bold}=> Checking dependencies…${reset} `);
const missing = [];

if (!hasCommand("cargo")) missing.push("cargo (rustup.rs)");
if (!hasCommand("node")) missing.push("node");
if (!hasCommand("docker")) missing.push("docker");

if (!run("cargo watch --version")) {
  log(`\n${yellow}=> Installing cargo-watch…${reset} `);
  if (run("cargo install cargo-watch --quiet")) {
    log(`${green}done${reset}\n`);
  } else {
    missing.push("cargo-watch (cargo install cargo-watch)");
  }
}

if (missing.length > 0) {
  log(`${red}failed${reset}\n`);
  console.error(`${red}Missing dependencies:${reset}`);
  for (const dep of missing) console.error(`  - ${dep}`);
  process.exit(1);
}
log(`${green}ok${reset}\n`);

// --- Start infra ---

log(`${bold}=> Starting infra (postgres, prometheus, grafana)…${reset} `);
if (!run(`${compose} up -d postgres prometheus grafana`)) {
  log(`${red}failed${reset}\n`);
  try {
    const out = runOutput(`${compose} up -d postgres prometheus grafana 2>&1`);
    console.error(out);
  } catch (e) {
    console.error(e.stdout || e.message);
  }
  process.exit(1);
}
log(`${green}done${reset}\n`);

// --- Wait for postgres ---

log(`${bold}=> Waiting for postgres…${reset} `);
let pgReady = false;
for (let i = 1; i <= 30; i++) {
  if (run(`${compose} exec -T postgres pg_isready -U regelrecht -d regelrecht_pipeline`)) {
    pgReady = true;
    break;
  }
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1000);
}
if (!pgReady) {
  log(`${red}timeout${reset}\n`);
  process.exit(1);
}
log(`${green}ready${reset}\n`);

// --- Install frontend deps if needed ---

let adminFe = true;
let editorFe = true;

if (!existsSync(join(root, "packages/admin/frontend-src/node_modules"))) {
  log(`${bold}=> Installing admin frontend deps…${reset} `);
  if (run("npm ci --silent", { cwd: join(root, "packages/admin/frontend-src") })) {
    log(`${green}done${reset}\n`);
  } else {
    log(`${yellow}skipped${reset} (set GITHUB_TOKEN in .env for private packages)\n`);
    adminFe = false;
  }
}

if (!existsSync(join(root, "frontend/node_modules"))) {
  log(`${bold}=> Installing editor frontend deps…${reset} `);
  if (run("npm ci --silent", { cwd: join(root, "frontend") })) {
    log(`${green}done${reset}\n`);
  } else {
    log(`${yellow}skipped${reset} (set GITHUB_TOKEN in .env for private packages)\n`);
    editorFe = false;
  }
}

// --- Spawn services detached ---

const pids = [];

function spawnDetached(label, cmd, args, opts = {}) {
  const fd = openSync(join(root, opts.logFile), "w");
  try {
    // On Windows with shell:true, child.pid is the cmd.exe wrapper PID.
    // dev-down.mjs uses taskkill /T (kill tree) to handle this.
    const child = spawn(cmd, args, {
      cwd: opts.cwd || root,
      env: { ...process.env, ...opts.env },
      stdio: ["ignore", fd, fd],
      detached: true,
      shell: true,
      windowsHide: true,
    });

    child.unref();

    if (child.pid) {
      pids.push(child.pid);
      log(`${bold}=> Started ${label} (PID ${child.pid})${reset}\n`);
    } else {
      log(`${red}=> Failed to start ${label}${reset}\n`);
    }
  } finally {
    closeSync(fd);
  }
}

const dbHost = env("DB_HOST", "localhost");
const pgPort = env("POSTGRES_PORT", "5433");
const rustLog = env("RUST_LOG", "info");
const dbUrl = `postgres://regelrecht:regelrecht_dev@${dbHost}:${pgPort}/regelrecht_pipeline`;

spawnDetached("admin API (cargo watch on :8000)", "cargo", [
  "watch", "-C", "packages", "-x", "run --package regelrecht-admin",
], {
  logFile: ".dev-admin.log",
  env: { DATABASE_URL: dbUrl, RUST_LOG: rustLog },
});

if (adminFe) {
  spawnDetached("admin frontend (vite on :3001)", "npx", ["vite"], {
    cwd: join(root, "packages/admin/frontend-src"),
    logFile: ".dev-admin-frontend.log",
  });
}

if (editorFe) {
  spawnDetached("editor frontend (vite on :3000)", "npx", ["vite"], {
    cwd: join(root, "frontend"),
    logFile: ".dev-editor.log",
  });
}

// --- Write pidfile ---

writeFileSync(join(root, ".dev-pids"), pids.join("\n") + "\n");

// --- Print status ---

const grafanaPort = env("GRAFANA_PORT", "3002");
const promPort = env("PROMETHEUS_PORT", "9090");

console.log("");
console.log(`${bold}${green}  Dev stack is running with hot reload${reset}`);
console.log("");
if (editorFe) {
  console.log("  Editor:     http://localhost:3000     (hot reload)");
}
if (adminFe) {
  console.log("  Admin UI:   http://localhost:3001     (hot reload, proxies API to :8000)");
}
console.log("  Admin API:  http://localhost:8000     (auto-recompile on save)");
console.log(`  Grafana:    http://localhost:${grafanaPort}`);
console.log(`  Prometheus: http://localhost:${promPort}`);
console.log(`  PostgreSQL: localhost:${pgPort}`);
console.log("");
console.log(`  ${dim}Admin API log:${reset}      tail -f .dev-admin.log`);
if (adminFe) {
  console.log(`  ${dim}Admin frontend log:${reset} tail -f .dev-admin-frontend.log`);
}
if (editorFe) {
  console.log(`  ${dim}Editor log:${reset}         tail -f .dev-editor.log`);
}
console.log(`  ${dim}Infra logs:${reset}         just dev-logs`);
console.log(`  ${dim}Database:${reset}           just dev-psql`);
console.log(`  ${dim}Stop everything:${reset}    just dev-down`);
