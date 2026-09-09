/**
 * Engine value helpers shared by the editor and the demo (RFC-036).
 *
 * The engine knows two kinds of "nothing". `null` is absence: the register is
 * authoritative and says there is none (no partner, no rent). Unknown is a
 * fact that exists but nobody has supplied: a `source: {}` input without data,
 * or an optional parameter the caller did not pass. Unknown reaches JS as the
 * object
 *
 *     { "__unknown": true, "missing": [{ "law": "...", "name": "...", "kind": "no_data" | "not_passed" }] }
 *
 * with a sentinel key like `__untranslatable`. One definition lives here so
 * the runner, the editor and the demo cannot disagree on the shape.
 */

/** True when `value` is the engine's Unknown value. */
export function isUnknown(value) {
  return (
    value !== null
    && typeof value === 'object'
    && !Array.isArray(value)
    && value.__unknown === true
  );
}

/**
 * The missing facts an Unknown value carries, each `{ law, name, kind }`.
 * An empty array for any other value, so callers need no `isUnknown` guard.
 */
export function missingFacts(value) {
  if (!isUnknown(value) || !Array.isArray(value.missing)) return [];
  return value.missing;
}
