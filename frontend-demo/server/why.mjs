/**
 * The "why" backend of the demo: a plain-language explanation of one outcome.
 *
 *   GET  /api/why         → 200 { available: true }        the button may show
 *   POST /api/why/check   → 204 | 401                        is this password right
 *   POST /api/why         → 200 text/plain, streamed         the explanation
 *                           401 wrong password, 400 bad body,
 *                           429 too many at once, 502 the model failed
 *
 * Every POST carries the password in `X-Demo-Password`. The demo is public and
 * the model runs on one Claude subscription, so the password is what keeps the
 * button to the people it was handed to. It is checked here and only here: a
 * check in the browser is one devtools edit away.
 *
 * The model runs through the Claude Code CLI (`claude -p`), the same route as
 * the poc-portal's policy assistant: with `CLAUDE_CODE_OAUTH_TOKEN` it runs on
 * a subscription, with `ANTHROPIC_API_KEY` on a Console key. A subscription
 * token is only usable through the CLI, which is why this is a server at all
 * and not a fetch from the browser.
 *
 * No tools, no settings, no session: the model reads the trace it is handed
 * and writes text. Nothing it says can touch the file system.
 */
import http from 'node:http';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn, execFile } from 'node:child_process';
import { randomBytes, scrypt, scryptSync, timingSafeEqual } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { buildRequest } from './prompt.mjs';

/** The request body carries a full trace; this is generous for that and no more. */
const MAX_BODY_BYTES = 1_000_000;

/** What the CLI child gets from our environment. The password stays out. */
const ENV_WHITELIST = [
  'PATH', 'HOME', 'TMPDIR', 'TZ', 'LANG', 'LC_ALL', 'USER',
  'CLAUDE_CODE_OAUTH_TOKEN', 'ANTHROPIC_API_KEY',
  'DISABLE_AUTOUPDATER', 'CLAUDE_CODE_SKIP_PROMPT_HISTORY',
];

/**
 * The shortest password the server starts with. There is no lockout after
 * wrong guesses: one shared across callers lets a single script keep the
 * presenter out for as long as it runs, and behind the platform's ingress
 * there is no caller address to scope one to. The length does the work a
 * lockout would: at 16 characters, guessing over the network is hopeless.
 */
export const MIN_PASSWORD_LENGTH = 16;

/**
 * A password checker. Both sides go through scrypt with a salt drawn at start,
 * so the comparison is constant-time and every guess costs the guesser a key
 * derivation; the expected key is derived once.
 */
export function passwordChecker(expected) {
  const salt = randomBytes(16);
  const want = scryptSync(String(expected), salt, 32);
  return (given) =>
    new Promise((resolve) => {
      if (!given) return resolve(false);
      scrypt(String(given), salt, 32, (err, got) => resolve(!err && timingSafeEqual(got, want)));
    });
}

function childEnv() {
  const env = {};
  for (const key of ENV_WHITELIST) if (process.env[key] !== undefined) env[key] = process.env[key];
  env.DISABLE_AUTOUPDATER ??= '1';
  env.CLAUDE_CODE_SKIP_PROMPT_HISTORY ??= 'true';
  return env;
}

function sendJson(res, status, body) {
  res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
  res.end(JSON.stringify(body));
}

async function readBody(req) {
  let size = 0;
  const chunks = [];
  for await (const chunk of req) {
    size += chunk.length;
    if (size > MAX_BODY_BYTES) return { tooLarge: true };
    chunks.push(chunk);
  }
  try {
    return { json: JSON.parse(Buffer.concat(chunks).toString('utf8')) };
  } catch {
    return { json: null };
  }
}

/**
 * The text a stream-json line from the CLI adds to the answer, if any.
 * `--include-partial-messages` sends the text as it is typed (text_delta);
 * the closing `result` line repeats all of it and is only used when no delta
 * came through.
 */
export function parseCliLine(line) {
  let event;
  try {
    event = JSON.parse(line);
  } catch {
    return null;
  }
  if (event.type === 'stream_event') {
    const delta = event.event?.delta;
    if (event.event?.type === 'content_block_delta' && delta?.type === 'text_delta') return { text: delta.text };
    return null;
  }
  if (event.type === 'result') return { result: event.result ?? '', isError: !!event.is_error };
  return null;
}

/**
 * @param {object} options
 * @param {string} options.password  the one password that unlocks the button
 * @param {string} [options.claudeBin]
 * @param {string} [options.model]
 * @param {number} [options.maxConcurrent]  CLI processes at once, on one subscription
 * @param {number} [options.timeoutMs]
 * @param {number} [options.wrongPasswordDelayMs]
 */
export function createWhyServer({
  password,
  claudeBin = 'claude',
  model = 'sonnet',
  maxConcurrent = 3,
  timeoutMs = 180_000,
  wrongPasswordDelayMs = 500,
}) {
  if (!password) throw new Error('a password is required: without one the button is open to everyone');
  let running = 0;
  const matches = passwordChecker(password);

  /** @returns {Promise<'ok' | 'wrong'>} */
  async function checkPassword(req) {
    if (await matches(req.headers['x-demo-password'])) return 'ok';
    // A pause on a wrong guess makes guessing slower, and costs a right one nothing.
    await new Promise((r) => setTimeout(r, wrongPasswordDelayMs));
    return 'wrong';
  }
  function refuse(res) {
    return sendJson(res, 401, { error: 'wrong password' });
  }

  async function explain(req, res) {
    const verdict = await checkPassword(req);
    if (verdict !== 'ok') return refuse(res);
    if (running >= maxConcurrent) return sendJson(res, 429, { error: 'too many explanations at once' });

    const { json, tooLarge } = await readBody(req);
    if (tooLarge) return sendJson(res, 413, { error: 'body too large' });
    const request = buildRequest(json);
    if (request.error) return sendJson(res, 400, { error: request.error });
    // Again, after the upload: requests that arrived together all passed the
    // first check while their bodies were still coming in.
    if (running >= maxConcurrent) return sendJson(res, 429, { error: 'too many explanations at once' });

    // An empty working directory: the CLI reads CLAUDE.md and .claude/ from
    // its cwd, and there is nothing here it should pick up. Created before the
    // slot is taken, so a failure here cannot leave the counter one too high.
    const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'demo-why-'));
    running += 1;
    const child = spawn(claudeBin, [
      '-p',
      '--output-format', 'stream-json',
      '--verbose',
      '--include-partial-messages',
      '--model', model,
      '--system-prompt', request.system,
      '--tools', '',
      '--setting-sources', '',
      '--disable-slash-commands',
      '--no-session-persistence',
      '--max-turns', '1',
    ], { cwd, env: childEnv(), stdio: ['pipe', 'pipe', 'pipe'] });

    let started = false; // headers sent: from here on errors can only end the stream
    let finished = false;
    let buffered = '';
    let stderr = '';

    const finish = (error) => {
      if (finished) return;
      finished = true;
      running -= 1;
      clearTimeout(timer);
      try { child.kill('SIGTERM'); } catch { /* already gone */ }
      fs.rm(cwd, { recursive: true, force: true }, () => {});
      if (res.writableEnded || res.destroyed) return;
      if (!started) return sendJson(res, 502, { error: error ?? 'no explanation' });
      // Cut off halfway: break the connection instead of ending it cleanly, so
      // the browser reports an error rather than showing half an explanation
      // as if it were the whole one.
      if (error) return res.destroy(new Error(error));
      res.end();
    };
    const write = (text) => {
      if (!text || res.writableEnded) return;
      if (!started) {
        started = true;
        res.writeHead(200, {
          'Content-Type': 'text/plain; charset=utf-8',
          'Cache-Control': 'no-store',
          // nginx would otherwise hold the stream until the answer is complete.
          'X-Accel-Buffering': 'no',
        });
      }
      res.write(text);
    };

    const timer = setTimeout(() => finish('the model took too long'), timeoutMs);
    // The visitor closed the sheet: the answer has no reader, and on one
    // subscription an orphaned process is not free.
    res.on('close', () => finish());

    let sawDelta = false;
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      buffered += chunk;
      let newline;
      while ((newline = buffered.indexOf('\n')) >= 0) {
        const line = buffered.slice(0, newline).trim();
        buffered = buffered.slice(newline + 1);
        if (!line) continue;
        const parsed = parseCliLine(line);
        if (!parsed) continue;
        if (parsed.text !== undefined) {
          sawDelta = true;
          write(parsed.text);
        } else if (parsed.isError) {
          finish(parsed.result || 'the model reported an error');
        } else if (!sawDelta) {
          write(parsed.result);
        }
      }
    });
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', (chunk) => {
      stderr = (stderr + chunk).slice(-2000);
    });
    child.on('error', (err) => finish(`the CLI did not start: ${err.message}`));
    child.on('close', (code) => {
      if (code !== 0 && !started) console.error(`why: claude exited ${code}: ${stderr.trim()}`);
      finish(code === 0 ? undefined : 'the model failed');
    });

    // The child can die while a long trace is still being written to it (the
    // sheet closed, the timeout hit). Without a listener that EPIPE is an
    // unhandled error, and it would take the whole server down.
    child.stdin.on('error', () => {});
    child.stdin.end(request.message);
  }

  return http.createServer(async (req, res) => {
    const url = new URL(req.url, 'http://localhost');
    try {
      if (url.pathname === '/api/why' && req.method === 'GET') return sendJson(res, 200, { available: true });
      if (url.pathname === '/api/why/check' && req.method === 'POST') {
        const verdict = await checkPassword(req);
        if (verdict !== 'ok') return refuse(res);
        res.writeHead(204).end();
        return;
      }
      if (url.pathname === '/api/why' && req.method === 'POST') return await explain(req, res);
      return sendJson(res, 404, { error: 'not found' });
    } catch (err) {
      console.error('why:', err);
      if (!res.headersSent) sendJson(res, 500, { error: 'internal error' });
      else res.end();
    }
  });
}

/** Whether the CLI answers at all; checked once at start so a broken image says so. */
function cliWorks(claudeBin) {
  return new Promise((resolve) => {
    execFile(claudeBin, ['--version'], { timeout: 10_000 }, (err) => resolve(!err));
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const password = process.env.DEMO_WHY_PASSWORD;
  if (!password) {
    console.error('why: DEMO_WHY_PASSWORD is not set; refusing to start an open endpoint');
    process.exit(1);
  }
  if (password.length < MIN_PASSWORD_LENGTH) {
    console.error(`why: DEMO_WHY_PASSWORD is shorter than ${MIN_PASSWORD_LENGTH} characters; a short one can be guessed`);
    process.exit(1);
  }
  const claudeBin = process.env.CLAUDE_BIN ?? 'claude';
  if (!(await cliWorks(claudeBin))) {
    console.error(`why: \`${claudeBin} --version\` fails; is the Claude Code CLI installed?`);
    process.exit(1);
  }
  const port = Number(process.env.PORT ?? 7401);
  const host = process.env.HOST ?? '127.0.0.1';
  createWhyServer({
    password,
    claudeBin,
    model: process.env.DEMO_WHY_MODEL ?? 'sonnet',
    maxConcurrent: Number(process.env.DEMO_WHY_MAX_CONCURRENT ?? 3),
  }).listen(port, host, () => console.log(`why: listening on ${host}:${port}`));
}
