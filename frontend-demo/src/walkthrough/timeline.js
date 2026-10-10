/**
 * The walkthrough's timeline, and the pure functions the player runs on it.
 *
 * `timeline.json` is written by post-processing (script/walkthrough/) and
 * copied into `/data/walkthrough/` with the rest of the demo corpus. The media
 * it names live under `/walkthrough/` and are not in git: the Docker build
 * fetches them from a GitHub release, pinned by checksum
 * (scripts/fetch-walkthrough-media.mjs).
 *
 * Shape, in seconds:
 *
 *   { version: 1, presenter: { name, role },
 *     main: TRACK,
 *     faq: [{ id, question, offer, ...TRACK }] }
 *
 *   TRACK = { duration, recordedAt, viewport,
 *             audio: { src },                    // the voice: the replay's clock
 *             video: { src, width, height },     // the window, for a phone and the MP4
 *             cam: { src } | null,               // the presenter bubble
 *             captions: { nl: 'file.vtt' },
 *             slides: [...],                     // the deck as recorded
 *             chapters: [{ start, end, slideIndex, slide, profile, state }],
 *             events: [{ t, type, ... }] }       // what replay.js does again
 *
 * Everything here is free of the DOM, so the player's decisions can be tested
 * without a browser.
 */

export const TIMELINE_URL = '/data/walkthrough/timeline.json';
export const MEDIA_BASE = '/walkthrough/';
export const CAPTIONS_BASE = '/data/walkthrough/';

/** The timeline, or null when this build carries no recording. */
export async function loadTimeline(fetchImpl = fetch) {
  try {
    const res = await fetchImpl(TIMELINE_URL);
    if (!res.ok) return null;
    const data = await res.json();
    return validTimeline(data) ? data : null;
  } catch {
    return null;
  }
}

export function validTrack(track) {
  return (
    !!track &&
    Number.isFinite(track.duration) &&
    typeof track.audio?.src === 'string' &&
    Array.isArray(track.events) &&
    Array.isArray(track.chapters) &&
    track.chapters.length > 0
  );
}

export function validTimeline(data) {
  return !!data && data.version === 1 && validTrack(data.main) && (data.faq ?? []).every((f) => typeof f.id === 'string' && validTrack(f));
}

/** Index of the chapter that plays at `t`; the last one past the end. */
export function chapterAt(track, t) {
  const chapters = track?.chapters ?? [];
  if (!chapters.length) return -1;
  for (let i = chapters.length - 1; i >= 0; i -= 1) {
    if (t >= chapters[i].start) return i;
  }
  return 0;
}

/**
 * Where "previous chapter" goes. Within the first seconds of a chapter it is
 * the one before; later it is the start of the current one, the way a music
 * player's back button works. Otherwise a single press would only ever restart.
 */
export function previousChapterStart(track, t, grace = 3) {
  const i = chapterAt(track, t);
  if (i < 0) return 0;
  const start = track.chapters[i].start;
  if (t - start > grace || i === 0) return start;
  return track.chapters[i - 1].start;
}

export function nextChapterStart(track, t) {
  const i = chapterAt(track, t);
  const next = track?.chapters?.[i + 1];
  return next ? next.start : null;
}

/** The FAQ entries offered by time `t`, in the order they appeared. */
export function offeredFaq(timeline, t) {
  return (timeline?.faq ?? []).filter((f) => Number.isFinite(f.offer) && f.offer <= t).sort((a, b) => a.offer - b.offer);
}

/** Clicks whose ripple is still visible at `t`. */
export function ripplesAt(clicks, t, length = 0.7) {
  return (clicks ?? []).filter((c) => c.t <= t && t - c.t < length);
}

/**
 * The box the video's picture actually occupies inside its element, with
 * `object-fit: contain`. A click is stored as a fraction of the picture, so its
 * ripple has to land on the picture and not on the letterbox around it.
 */
export function fitRect(boxW, boxH, mediaW, mediaH) {
  if (!(boxW > 0 && boxH > 0 && mediaW > 0 && mediaH > 0)) return { left: 0, top: 0, width: boxW || 0, height: boxH || 0 };
  const scale = Math.min(boxW / mediaW, boxH / mediaH);
  const width = mediaW * scale;
  const height = mediaH * scale;
  return { left: (boxW - width) / 2, top: (boxH - height) / 2, width, height };
}

/**
 * How a follower (the webcam) should catch up with the master (the app
 * video). A small drift is nudged with the playback rate, which is invisible;
 * a large one (after a seek, a stall) is a jump.
 */
export function followerCorrection(masterT, followerT, baseRate = 1) {
  const drift = followerT - masterT;
  if (Math.abs(drift) > 0.3) return { seek: masterT, rate: baseRate };
  if (Math.abs(drift) > 0.04) return { rate: baseRate * (drift > 0 ? 0.95 : 1.05) };
  return { rate: baseRate };
}

function vttTime(s) {
  const m = /^(?:(\d+):)?(\d{1,2}):(\d{2})[.,](\d{3})$/.exec(s.trim());
  if (!m) return NaN;
  return Number(m[1] ?? 0) * 3600 + Number(m[2]) * 60 + Number(m[3]) + Number(m[4]) / 1000;
}

/**
 * The cues of a WebVTT file. Only what post-processing writes: a header, then
 * blocks of an optional id, a timing line and text. Settings after the timing
 * are ignored, and so are NOTE and STYLE blocks.
 */
export function parseVtt(text) {
  const cues = [];
  const blocks = String(text ?? '')
    .replace(/\r\n?/g, '\n')
    .split(/\n{2,}/);
  for (const block of blocks) {
    const lines = block.split('\n').filter((l) => l.length);
    const timing = lines.findIndex((l) => l.includes('-->'));
    if (timing < 0) continue;
    const [a, b] = lines[timing].split('-->');
    const start = vttTime(a);
    const end = vttTime(b.trim().split(/\s+/)[0]);
    if (!Number.isFinite(start) || !Number.isFinite(end)) continue;
    cues.push({ start, end, text: lines.slice(timing + 1).join('\n') });
  }
  return cues;
}

export function cueAt(cues, t) {
  return (cues ?? []).find((c) => t >= c.start && t < c.end) ?? null;
}

/** `m:ss`, or `h:mm:ss` from an hour. */
export function formatTime(seconds) {
  const s = Math.max(0, Math.floor(seconds || 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = String(s % 60).padStart(2, '0');
  return h ? `${h}:${String(m).padStart(2, '0')}:${r}` : `${m}:${r}`;
}

/**
 * Whether the presenter's bubble shows at `t`. The webcam covers the recorded
 * opening (`cam.until`); where the generated voice takes over there is no
 * picture of the presenter, so the bubble goes.
 */
export function camVisible(track, t) {
  return !!track?.cam && (track.cam.until == null || t < track.cam.until);
}

/** The most recent `profile` at or before chapter `i`, as the deck does it. */
export function profileAt(track, i) {
  for (let j = i; j >= 0; j -= 1) {
    const p = track?.chapters?.[j]?.profile ?? track?.chapters?.[j]?.slide?.profile;
    if (p) return p;
  }
  return null;
}

/**
 * The pause the clock just reached, or null: one with `prev < t <= now`.
 * Only for a clock that ran (a tick of a few frames); a jump (a seek, a
 * chapter) passes pauses without stopping at them.
 */
export function pauseDue(pauses, prev, now, maxStep = 1) {
  if (!(now > prev) || now - prev > maxStep) return null;
  return (pauses ?? []).find((p) => p.t > prev && p.t <= now) ?? null;
}

/** One of `clips` at random, or null. */
export function pickClip(clips, random = Math.random) {
  return clips?.length ? clips[Math.floor(random() * clips.length) % clips.length] : null;
}
