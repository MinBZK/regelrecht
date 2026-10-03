/**
 * Fetch the walkthrough's video files into public/walkthrough/.
 *
 * The timeline and the captions are in git (corpus/demo/walkthrough/); the
 * videos are not, because a re-recorded chapter would leave tens of
 * megabytes in the history for good. They are assets of a GitHub release,
 * named in `timeline.json` together with their SHA-256. This script downloads
 * each one and refuses a file whose checksum does not match: what the image
 * serves is exactly what was built from the takes, or the build fails.
 *
 * Runs in the Docker build, before `npm run build`. Without a timeline there
 * is no walkthrough and nothing to fetch. A file that is already in place with
 * the right checksum (after `walkthrough build` on a laptop) is left alone.
 *
 * WALKTHROUGH_MEDIA_BASE overrides where the files come from, for a test.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(here, '..');
const timelinePath = resolve(appRoot, '..', 'corpus', 'demo', 'walkthrough', 'timeline.json');
const dest = resolve(appRoot, 'public', 'walkthrough');
const REPO = 'MinBZK/regelrecht';

/** Every media file the timeline names, with its checksum. */
export function mediaFiles(timeline) {
  const tracks = [timeline.main, ...(timeline.faq ?? [])];
  return tracks.flatMap((t) => [t.audio, t.video, t.cam].filter(Boolean)).map(({ src, sha256 }) => ({ src, sha256 }));
}

function sha256(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

async function main() {
  if (!existsSync(timelinePath)) {
    console.log('walkthrough: geen timeline.json, niets op te halen');
    return;
  }
  const timeline = JSON.parse(readFileSync(timelinePath, 'utf8'));
  const base = process.env.WALKTHROUGH_MEDIA_BASE ?? (timeline.release ? `https://github.com/${REPO}/releases/download/${timeline.release}/` : null);
  if (!base) throw new Error('walkthrough: timeline.json noemt geen `release`, en WALKTHROUGH_MEDIA_BASE is niet gezet');
  mkdirSync(dest, { recursive: true });
  for (const { src, sha256: want } of mediaFiles(timeline)) {
    if (!/^[A-Za-z0-9._-]+$/.test(src)) throw new Error(`walkthrough: ongeldige bestandsnaam ${src}`);
    const target = join(dest, src);
    if (existsSync(target) && sha256(readFileSync(target)) === want) continue;
    const res = await fetch(`${base}${src}`);
    if (!res.ok) throw new Error(`walkthrough: ${src} ophalen gaf HTTP ${res.status}`);
    const buf = Buffer.from(await res.arrayBuffer());
    const got = sha256(buf);
    if (got !== want) throw new Error(`walkthrough: ${src} heeft checksum ${got}, verwacht ${want}`);
    writeFileSync(target, buf);
    console.log(`walkthrough: ${src} (${(buf.length / 1e6).toFixed(1)} MB)`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((e) => {
    console.error(e.message);
    process.exit(1);
  });
}
