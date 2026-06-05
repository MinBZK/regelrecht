/**
 * useTrajectDetail — fetch one traject's full detail (`GET /api/trajects/:id`),
 * including its `sources`, for the read-only Traject-info sheet.
 *
 * Same endpoint and fresh-state-per-call shape as `useTrajectMembers`, but
 * keeps the whole `TrajectDetail` object (that composable discards `sources`).
 * The `id` argument is the traject **UUID** — the same value
 * `TrajectMenu.openMembersForActive` passes for member management — not the
 * URL `ref` form.
 */
import { ref } from 'vue';

/**
 * Pick the writable-own source from a `TrajectDetail`. That is the source the
 * traject pushes edits to, so its repo/branch fields are what the info sheet
 * shows. Returns `null` when there is no detail or no writable source (a
 * defensive shape the backend shouldn't produce, but the UI must not crash on).
 */
export function writableSource(detail) {
  if (!detail || !Array.isArray(detail.sources)) return null;
  return detail.sources.find((s) => s.is_writable_own) ?? null;
}

/**
 * Build a GitHub tree URL pointing at the traject branch:
 * `https://github.com/{owner}/{repo}/tree/{branch}`. Slashes inside the branch
 * name are left intact (GitHub serves `tree/feature/x` directly; percent-
 * encoding the `/` would break it). Returns `null` when the source is missing
 * owner/repo/branch, so the caller can render plain text instead of a dead link.
 */
export function branchTreeUrl(source) {
  if (!source) return null;
  const { gh_owner, gh_repo, gh_branch } = source;
  if (!gh_owner || !gh_repo || !gh_branch) return null;
  return `https://github.com/${gh_owner}/${gh_repo}/tree/${gh_branch}`;
}

export function useTrajectDetail() {
  const detail = ref(null);
  const loading = ref(false);
  const error = ref(null);

  async function load(trajectId) {
    // Reset before the await so a reopen against a different traject can't
    // briefly flash the previous traject's data (mirrors useTrajectMembers).
    loading.value = true;
    error.value = null;
    detail.value = null;
    try {
      const resp = await fetch(`/api/trajects/${trajectId}`);
      if (!resp.ok) {
        throw new Error(`Kon traject niet laden: ${resp.status}`);
      }
      detail.value = await resp.json();
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  return { detail, loading, error, load };
}
