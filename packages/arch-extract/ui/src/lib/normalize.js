/**
 * normalize — put every prototype's layout in the same world box.
 *
 * The three prototypes each compute their own geometry (dagre ranks, a ring, a
 * matrix), and each *level* of each prototype has a different natural size. If
 * those coordinate spaces differed, changing level would make the picture jump
 * and the point under the cursor would slide away.
 *
 * So every layout is scaled — aspect ratio preserved — into one fixed square
 * centred on the origin. Two consequences that the acceptance criteria depend
 * on:
 *
 *  - the view transform (pan + zoom) survives a level change untouched, so the
 *    world point under the cursor stays exactly where it was (criterion 9);
 *  - the zoom factor means "×the whole model fits" in every prototype and at
 *    every level, so one set of zoom thresholds fits all (criterion 7).
 */

/** Side of the shared world square; coordinates run from −500 to +500. */
export const WORLD_SIZE = 1000;

/**
 * Scale + translate `points` so their bounding box fits the world square.
 *
 * @param {Array<{x:number,y:number}>} points  mutated in place
 * @param {Array<Array<{x:number,y:number}>>} [polylines]  extra point arrays to
 *   transform with the same factors (edge paths)
 * @returns {{ scale:number, offsetX:number, offsetY:number }}
 */
export function fitToWorld(points, polylines = []) {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of points) {
    if (p.x < minX) minX = p.x;
    if (p.y < minY) minY = p.y;
    if (p.x > maxX) maxX = p.x;
    if (p.y > maxY) maxY = p.y;
  }
  if (!Number.isFinite(minX)) return { scale: 1, offsetX: 0, offsetY: 0 };

  const w = Math.max(1e-6, maxX - minX);
  const h = Math.max(1e-6, maxY - minY);
  const scale = WORLD_SIZE / Math.max(w, h);
  const offsetX = -(minX + w / 2) * scale;
  const offsetY = -(minY + h / 2) * scale;

  const apply = (p) => {
    p.x = p.x * scale + offsetX;
    p.y = p.y * scale + offsetY;
  };
  for (const p of points) apply(p);
  for (const line of polylines) for (const p of line) apply(p);

  return { scale, offsetX, offsetY };
}

/** Bounding box of a set of points, in world coordinates. */
export function boundsOf(points) {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of points) {
    if (p.x < minX) minX = p.x;
    if (p.y < minY) minY = p.y;
    if (p.x > maxX) maxX = p.x;
    if (p.y > maxY) maxY = p.y;
  }
  if (!Number.isFinite(minX)) {
    return { minX: -WORLD_SIZE / 2, minY: -WORLD_SIZE / 2, maxX: WORLD_SIZE / 2, maxY: WORLD_SIZE / 2 };
  }
  return { minX, minY, maxX, maxY };
}
