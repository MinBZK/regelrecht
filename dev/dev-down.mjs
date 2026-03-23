#!/usr/bin/env node
// Cross-platform dev shutdown
// Replaces the bash-only `just dev-down` recipe

import { execSync } from "node:child_process";
import { readFileSync, existsSync, unlinkSync } from "node:fs";
import { resolve, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { platform } from "node:os";

const isWindows = platform() === "win32";
const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");

const bold = "\x1b[1m";
const reset = "\x1b[0m";
const green = "\x1b[32m";

const compose = "docker compose -f docker-compose.dev.yml -f dev/compose.native.yaml";
const pidfile = join(root, ".dev-pids");

function log(msg) {
  process.stdout.write(msg);
}

function tryExec(cmd) {
  try {
    execSync(cmd, { stdio: "ignore", cwd: root });
  } catch {
    // ignore
  }
}

// --- Kill native services ---

log(`${bold}=> Stopping native services…${reset} `);

if (existsSync(pidfile)) {
  const pids = readFileSync(pidfile, "utf8")
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);

  for (const pid of pids) {
    if (!/^\d+$/.test(pid)) continue;
    if (isWindows) {
      tryExec(`taskkill /F /T /PID ${pid}`);
    } else {
      // Negative PID sends signal to entire process group (detached procs get own PGID)
      tryExec(`kill -TERM -${pid}`);
    }
  }

  unlinkSync(pidfile);

  // Brief grace period for processes to exit (Windows locks open log files)
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1000);
}

// Cleanup log files (try/catch: Windows may still lock files briefly)
for (const f of [".dev-admin.log", ".dev-admin-frontend.log", ".dev-editor.log"]) {
  const p = join(root, f);
  try { if (existsSync(p)) unlinkSync(p); } catch { /* file still in use */ }
}

log(`${green}done${reset}\n`);

// --- Stop infra ---

log(`${bold}=> Stopping infra…${reset} `);
tryExec(`${compose} down`);
log(`${green}done${reset}\n`);
