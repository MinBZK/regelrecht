/**
 * The editor-api's version-selection rule, in JS.
 *
 * A mirror of `extract_date_from_path` / `pick_best_version` in
 * `packages/corpus/src/source_map.rs`. The e2e corpus mock serves laws straight
 * off disk, so it has to collapse a law's versions exactly as the server does —
 * otherwise a spec silently tests a version the editor would never be handed.
 * Keep both sides in step: a change to the Rust rule belongs here too.
 */

/** Today as YYYY-MM-DD, matching the server's UTC `today_str()`. */
export function todayIso() {
  return new Date().toISOString().slice(0, 10);
}

/**
 * Extract a YYYY-MM-DD date from the FILENAME of a path (the corpus convention
 * is `…/{law_id}/{valid_from}.yaml`). The body's `publication_date` is a
 * different fact and deliberately plays no part. Returns null for anything that
 * isn't a dated `.yaml`.
 */
export function extractDateFromPath(path) {
  const filename = String(path ?? '').split('/').pop();
  if (!filename.endsWith('.yaml')) return null;
  const stem = filename.slice(0, -'.yaml'.length);
  return /^\d{4}-\d{2}-\d{2}$/.test(stem) ? stem : null;
}

/**
 * Whether `candidate` should replace `existing`, both as filename dates
 * (null = undated). A version in force today beats a future one; within either
 * group the later date wins; a dated file always beats an undated one.
 */
export function pickBestVersion(existing, candidate, today) {
  if (existing == null) return candidate != null;
  if (candidate == null) return false;
  const existingInForce = existing <= today;
  const candidateInForce = candidate <= today;
  if (existingInForce === candidateInForce) return candidate > existing;
  return candidateInForce;
}

/**
 * Comparator for the `/versions` contract: newest filename date first, undated
 * entries last (`insert_version`'s `sort_by_cached_key(Reverse(...))`).
 */
export function compareVersionsNewestFirst(pathA, pathB) {
  const a = extractDateFromPath(pathA);
  const b = extractDateFromPath(pathB);
  if (a === b) return 0;
  if (a == null) return 1;
  if (b == null) return -1;
  return a < b ? 1 : -1;
}
