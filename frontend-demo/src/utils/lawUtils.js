// Shared helpers for working with raw law-YAML in the browser.
//
// Both useEngine.js (main thread) and workers/simulator.js need to extract a
// law's `$id` to decide whether to register it with the WASM engine. Keeping
// the regex in one place means a future change (e.g. handling quoted `$id`
// values) only has to land here.

/**
 * Quick `$id` extractor that avoids parsing the full YAML.
 *
 * Recognises:
 *   $id: zorgtoeslagwet
 *   $id:   wet_op_de_zorgtoeslag
 *
 * Returns the id string, or an empty string if no `$id` is present.
 *
 * @param {string} yaml - Raw YAML text
 * @returns {string}
 */
export function deriveLawId(yaml) {
  const match = yaml.match(/^\$id:\s*(\S+)/m);
  return match ? match[1] : '';
}
