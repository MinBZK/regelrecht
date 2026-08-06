/**
 * Nodes as one InstancedMesh per geometry family.
 *
 * Six families, so the whole corpus draws in six calls no matter how many
 * laws there are. What costs money is writing the per-instance buffers, so
 * they are written once at build time and never per frame:
 *
 * - position lives in `instanceMatrix` (translation only, never rewritten),
 * - size lives in a per-instance `aScale` float and is mixed in the shader,
 *   so switching between structure mode and weight mode is a uniform change
 *   and not a rewrite of 100.000 matrices,
 * - hover and selection live in a per-instance `aState` byte, so a hover
 *   touches the node and its neighbours instead of every node in the graph.
 *
 * Dimming is a colour blend towards the background in the shader, not alpha:
 * transparent instances would force depth sorting, which at this instance
 * count is more expensive than everything else in the frame put together.
 */

import {
  BoxGeometry,
  CylinderGeometry,
  Color,
  InstancedBufferAttribute,
  InstancedMesh,
  Matrix4,
  OctahedronGeometry,
  Vector2,
  ShaderMaterial,
  SphereGeometry,
  TetrahedronGeometry,
} from 'three';
import { GEOMETRY_FAMILIES } from './graphSchema.js';
import { nodeColor, colorToLinearRgb } from './palette.js';

/**
 * Visibility floor, as an on-screen radius in device pixels.
 *
 * Below roughly one device pixel across a node stops being small and starts
 * being absent: without multisampling a sub-pixel triangle either misses the
 * sample point entirely or catches it, so the node blinks as the camera turns,
 * and multisampling is off above 60.000 nodes precisely where the nodes are
 * smallest. 0,75 keeps a node one and a half pixels across, which on a normal
 * screen is a dot you can see and on a HiDPI screen still a fine one.
 */
export const MIN_NODE_PIXELS = 0.75;

/**
 * The same floor for the id pass, in device pixels of radius. Larger, because
 * a target you cannot hit is worse than a dot you cannot see.
 */
export const MIN_PICK_PIXELS = 3.0;

export const STATE_NORMAL = 0;
export const STATE_HIGHLIGHT = 1;
export const STATE_SELECTED = 2;
export const STATE_GHOST = 3;

const NODE_VERT = /* glsl */ `
  attribute vec3 aColor;
  attribute vec3 aPick;
  attribute float aScale;
  attribute float aState;

  uniform float uWeightMix;
  uniform float uBaseSize;
  uniform float uPixelScale;
  uniform float uMinPixels;

  varying vec3 vColor;
  varying vec3 vNormalW;
  varying float vState;
  varying float vDepth;

  void main() {
    // World size first, then the visibility floor. Sized off the density the
    // node is small enough that at "hele graaf in beeld" it can fall below a
    // pixel, and a triangle smaller than a pixel does not go faint - it
    // flickers on and off with the camera. The floor is stated in device
    // pixels and converted to world units at this instance's own depth, so it
    // only bites where the node would otherwise disappear and lets the real
    // size take over the moment you zoom in.
    float worldSize = uBaseSize * mix(1.0, aScale, uWeightMix);
    float viewZ = max(-(modelViewMatrix * instanceMatrix * vec4(0.0, 0.0, 0.0, 1.0)).z, 1e-6);
    float s = max(worldSize, uMinPixels * uPixelScale * viewZ);
    vec3 local = position * s;
    vec4 world = instanceMatrix * vec4(local, 1.0);
    vNormalW = normalize(mat3(instanceMatrix) * normal);
    vColor = aColor;
    vState = aState;
    vec4 viewPos = modelViewMatrix * world;
    vDepth = -viewPos.z;
    gl_Position = projectionMatrix * viewPos;
  }
`;

const NODE_FRAG = /* glsl */ `
  precision mediump float;

  uniform vec3 uBackground;
  uniform vec3 uSelection;
  uniform float uDimOthers;
  uniform vec3 uFogColor;
  uniform vec2 uFogRange;

  varying vec3 vColor;
  varying vec3 vNormalW;
  varying float vState;
  varying float vDepth;

  void main() {
    // Two-band hemisphere light: enough to read the silhouette and the facets
    // of the non-spherical families, and it costs one dot product.
    vec3 n = normalize(vNormalW);
    float key = max(dot(n, normalize(vec3(0.4, 0.8, 0.55))), 0.0);
    float fill = 0.45 + 0.55 * key;
    vec3 c = vColor * fill;

    if (vState > 1.5 && vState < 2.5) {
      c = mix(c, uSelection, 0.65);
    } else if (vState > 2.5) {
      c = mix(c, uBackground, 0.9);
    } else if (uDimOthers > 0.5 && vState < 0.5) {
      // Dimmed, not erased. On a grey corpus against a white background the
      // usual 15%-opacity dim leaves nothing at all to orient by, and losing
      // the map is a worse trade than a slightly less loud selection.
      c = mix(c, uBackground, 0.7);
    }

    // Depth cue. A field of four thousand grey nodes without it is one flat
    // cloud; fading the far side into the background gives the eye the
    // structure that colour is not allowed to give here.
    // Capped at roughly half: a depth cue, not a veil. Beyond that the front
    // of a grey field washes out too and the picture reads as fog instead of
    // as a corpus.
    float fog = smoothstep(uFogRange.x, uFogRange.y, vDepth);
    c = mix(c, uFogColor, fog * 0.55);

    gl_FragColor = vec4(c, 1.0);
    // three converts the working space to the output colour space only in its
    // own materials; a hand-written ShaderMaterial has to include the chunk
    // itself, or every node renders darker than its token says.
    #include <colorspace_fragment>
  }
`;

const PICK_VERT = /* glsl */ `
  precision highp float;
  attribute vec3 aPick;
  attribute float aScale;
  uniform float uWeightMix;
  uniform float uBaseSize;
  uniform float uPickInflate;
  uniform float uPixelScale;
  uniform float uPickMinPixels;
  varying vec3 vPick;
  void main() {
    // The id pass gets its own, larger floor: a node drawn one pixel wide is
    // readable as a field but not hittable with a mouse, and a hover target
    // that small would make the whole map feel broken.
    float s = uBaseSize * mix(1.0, aScale, uWeightMix) * uPickInflate;
    float viewZ = max(-(modelViewMatrix * instanceMatrix * vec4(0.0, 0.0, 0.0, 1.0)).z, 1e-6);
    s = max(s, uPickMinPixels * uPixelScale * viewZ);
    vec4 world = instanceMatrix * vec4(position * s, 1.0);
    vPick = aPick;
    gl_Position = projectionMatrix * modelViewMatrix * world;
  }
`;

const PICK_FRAG = /* glsl */ `
  precision highp float;
  varying vec3 vPick;
  // No colour-space conversion here on purpose: this pass writes ids, not
  // colours, and a conversion would corrupt every one of them.
  void main() { gl_FragColor = vec4(vPick, 1.0); }
`;

/** Low-poly geometry per family; detail drops as the graph grows. */
function familyGeometry(name, lod) {
  const seg = lod === 'high' ? [12, 8] : lod === 'mid' ? [8, 6] : [6, 4];
  switch (name) {
    case 'box':
      return new BoxGeometry(1.6, 1.6, 1.6);
    case 'octahedron':
      return new OctahedronGeometry(1.05, 0);
    case 'tetrahedron':
      return new TetrahedronGeometry(1.2, 0);
    case 'cylinder':
      return new CylinderGeometry(1.0, 1.0, 0.5, lod === 'low' ? 6 : 10);
    case 'smallSphere':
      return new SphereGeometry(0.5, seg[0], seg[1]);
    case 'sphere':
    default:
      return new SphereGeometry(1, seg[0], seg[1]);
  }
}

export function lodForCount(nodeCount) {
  if (nodeCount <= 8000) return 'high';
  if (nodeCount <= 60000) return 'mid';
  return 'low';
}

/**
 * Encode an instance index into a pick colour. Index 0 is reserved for
 * "nothing", so the id written to the buffer is `index + 1` - a black pixel
 * then unambiguously means the background.
 */
export function encodePickId(index) {
  const id = index + 1;
  return [((id >> 16) & 0xff) / 255, ((id >> 8) & 0xff) / 255, (id & 0xff) / 255];
}

/** Inverse of `encodePickId` for a read-back RGB byte triple. */
export function decodePickId(r, g, b) {
  const id = (r << 16) | (g << 8) | b;
  return id === 0 ? -1 : id - 1;
}

/** Smallest node scale in weight mode, as a factor on the base size. */
export const WEIGHT_SCALE_MIN = 0.75;
/** Largest node scale in weight mode. */
export const WEIGHT_SCALE_MAX = 2.0;

/**
 * Weight -> radius, as a factor on the base size.
 *
 * Logarithmic and capped at under 3:1 between the heaviest and the lightest
 * node. The range is centred on 1 rather than starting there: the base size is
 * calibrated on the spacing of the layout, so a weight factor that only ever
 * multiplies would push every node past the spacing it was fitted to. Weight
 * modulates the size; it does not set it.
 */
export function weightScale(weight, minWeight, maxWeight) {
  const lo = Math.log(1 + Math.max(0, minWeight));
  const hi = Math.log(1 + Math.max(0, maxWeight));
  if (!(hi > lo)) return 1;
  const t = (Math.log(1 + Math.max(0, weight)) - lo) / (hi - lo);
  return (
    WEIGHT_SCALE_MIN + (WEIGHT_SCALE_MAX - WEIGHT_SCALE_MIN) * Math.min(1, Math.max(0, t))
  );
}

/**
 * Node radius as a fraction of the typical nearest-neighbour distance.
 *
 * At 0.22 a node of average weight is roughly a third of the way to its
 * nearest neighbour and the heaviest one (x2, see `WEIGHT_SCALE_MAX`) still
 * does not touch it. Everything larger than that closes the gaps between
 * neighbours, and once the gaps are closed the corpus reads as one solid body
 * whatever the layout underneath it says.
 */
export const NODE_RADIUS_FRACTION = 0.22;

/**
 * Distance from a node to its nearest neighbour, at a low quantile over a
 * deterministic sample.
 *
 * This is the number the node size has to follow, and it is not the same thing
 * as the size of the cloud. The layout puts a few framework laws far outside
 * the body of the corpus, so the bounding radius is twice what the corpus
 * itself spans, and it says nothing at all about whether two nodes touch. The
 * mean spacing `2R/cbrt(N)` inherits both problems: it is inflated by the
 * outliers and it assumes the nodes are spread evenly, while a force layout is
 * dense in the middle by construction. Measured on the real corpus the two
 * disagree by a factor of five at law level and twenty at article level, which
 * is exactly the amount by which the picture was too thick.
 *
 * A low quantile (the default keeps the densest quarter) rather than the median
 * because the size has to hold where the graph is dense; where it is sparse a
 * node that is smaller than it could be costs nothing.
 *
 * @param {Float32Array} positions xyz per node
 * @param {number} nodeCount
 * @param {object} [opts]
 * @returns {number} 0 when there is nothing to measure
 */
export function nearestNeighbourSpacing(
  positions,
  nodeCount,
  { sample = 4000, quantile = 0.25 } = {},
) {
  if (nodeCount < 2) return 0;

  let minX = Infinity;
  let minY = Infinity;
  let minZ = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  let maxZ = -Infinity;
  for (let i = 0; i < nodeCount; i++) {
    const x = positions[i * 3];
    const y = positions[i * 3 + 1];
    const z = positions[i * 3 + 2];
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) continue;
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (z < minZ) minZ = z;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
    if (z > maxZ) maxZ = z;
  }
  if (!Number.isFinite(minX)) return 0;
  const span = Math.max(maxX - minX, maxY - minY, maxZ - minZ);
  if (!(span > 0)) return 0;

  // Grid over the layout, refined until no cell holds more than a handful of
  // nodes. One pass at the average density is not enough: a force layout piles
  // most of the corpus into the middle, and a cell there would hold thousands
  // of nodes - which is either a scan of the whole core per sample, or a cap on
  // that scan that reports the distance to some arbitrary node in the cell
  // instead of to the nearest one. Shrinking the cell until the fullest one is
  // small keeps both the cost and the answer honest, at O(N) per pass and at
  // most a handful of passes.
  const MAX_PER_CELL = 64;
  let perAxis = Math.min(512, Math.max(2, Math.round(Math.cbrt(8 * nodeCount))));
  let cell = span / perAxis;
  let cellOf;
  let buckets;
  for (let pass = 0; ; pass++) {
    cell = span / perAxis;
    const axis = perAxis;
    const size = cell;
    cellOf = (v, lo) => Math.min(axis - 1, Math.max(0, Math.floor((v - lo) / size)));
    buckets = new Map();
    let fullest = 0;
    for (let i = 0; i < nodeCount; i++) {
      const key =
        (cellOf(positions[i * 3], minX) * axis + cellOf(positions[i * 3 + 1], minY)) * axis +
        cellOf(positions[i * 3 + 2], minZ);
      const bucket = buckets.get(key);
      if (bucket) {
        bucket.push(i);
        if (bucket.length > fullest) fullest = bucket.length;
      } else buckets.set(key, [i]);
    }
    if (fullest <= MAX_PER_CELL || perAxis >= 512 || pass >= 3) break;
    // Jump straight to the resolution the fullest cell asks for rather than
    // halving: at corpus scale every extra pass is another walk over 200.000
    // nodes, and this converges in one or two.
    perAxis = Math.min(
      512,
      Math.max(perAxis + 1, Math.ceil(perAxis * Math.cbrt(fullest / MAX_PER_CELL))),
    );
  }

  const step = Math.max(1, Math.floor(nodeCount / Math.max(1, sample)));
  const found = [];
  for (let i = 0; i < nodeCount; i += step) {
    const gx = cellOf(positions[i * 3], minX);
    const gy = cellOf(positions[i * 3 + 1], minY);
    const gz = cellOf(positions[i * 3 + 2], minZ);
    let best = Infinity;
    // Rings outward until the nearest hit is closer than the part of the grid
    // still unsearched. One ring is enough wherever the layout is dense, which
    // is where the answer comes from; the further rings only exist so a node in
    // a thin region is not silently dropped.
    for (let r = 1; r <= 4; r++) {
      for (let a = -r; a <= r; a++) {
        const x = gx + a;
        if (x < 0 || x >= perAxis) continue;
        for (let b = -r; b <= r; b++) {
          const y = gy + b;
          if (y < 0 || y >= perAxis) continue;
          for (let c = -r; c <= r; c++) {
            const z = gz + c;
            if (z < 0 || z >= perAxis) continue;
            const bucket = buckets.get((x * perAxis + y) * perAxis + z);
            if (!bucket) continue;
            // Cap the scan: a layout can pile thousands of nodes into one cell,
            // and an uncapped search would make this O(sample x N).
            const upto = Math.min(bucket.length, 256);
            for (let k = 0; k < upto; k++) {
              const j = bucket[k];
              if (j === i) continue;
              const dx = positions[j * 3] - positions[i * 3];
              const dy = positions[j * 3 + 1] - positions[i * 3 + 1];
              const dz = positions[j * 3 + 2] - positions[i * 3 + 2];
              const d2 = dx * dx + dy * dy + dz * dz;
              if (d2 < best) best = d2;
            }
          }
        }
      }
      // `best` is a squared distance: everything not yet searched lies at
      // least r cells away, so a hit closer than that is already the nearest.
      const reach = r * cell;
      if (best <= reach * reach) break;
    }
    // A sample with nothing within four cells of itself sits in empty space. It
    // is dropped rather than counted as "very far away": the size follows the
    // dense part of the corpus, not the strays around it.
    if (best < Infinity && best > 0) found.push(Math.sqrt(best));
  }
  if (found.length === 0) return 0;
  found.sort((a, b) => a - b);
  return found[Math.min(found.length - 1, Math.floor(quantile * (found.length - 1)))];
}

export class NodeLayer {
  /**
   * @param {import('./graphSchema.js').PackedGraph} graph
   * @param {object} palette
   * @param {object} [opts]
   */
  constructor(graph, palette, { baseSize = 1.2, weightMode = true } = {}) {
    this.graph = graph;
    this.palette = palette;
    this.meshes = [];
    /** instance index -> node index, per family */
    this.instanceToNode = [];
    /** node index -> [familyIndex, instanceIndex] */
    this.nodeToInstance = new Int32Array(graph.nodeCount * 2).fill(-1);
    this.baseSize = baseSize;
    this.lod = lodForCount(graph.nodeCount);
    /** family -> attribute -> {min, max}; flushed once per frame */
    this.dirty = new Map();

    const counts = new Uint32Array(GEOMETRY_FAMILIES.length);
    for (let i = 0; i < graph.nodeCount; i++) counts[graph.kind[i]]++;

    let minW = Infinity;
    let maxW = -Infinity;
    for (let i = 0; i < graph.nodeCount; i++) {
      const w = graph.weight[i];
      if (w < minW) minW = w;
      if (w > maxW) maxW = w;
    }
    this.minWeight = minW;
    this.maxWeight = maxW;

    const uniforms = {
      uWeightMix: { value: weightMode ? 1 : 0 },
      uBaseSize: { value: baseSize },
      uBackground: { value: new Color(palette.background) },
      uSelection: { value: new Color(palette.selection) },
      uDimOthers: { value: 0 },
      uPickInflate: { value: 1.6 },
      // World units per device pixel at unit view depth; set from the viewport
      // (see `setPixelScale`). Zero until then, which switches both floors off.
      uPixelScale: { value: 0 },
      uMinPixels: { value: MIN_NODE_PIXELS },
      uPickMinPixels: { value: MIN_PICK_PIXELS },
      uFogColor: { value: new Color(palette.background) },
      uFogRange: { value: new Vector2(1e9, 1e9 + 1) },
    };
    this.uniforms = uniforms;

    this.material = new ShaderMaterial({
      uniforms,
      vertexShader: NODE_VERT,
      fragmentShader: NODE_FRAG,
    });
    this.pickMaterial = new ShaderMaterial({
      uniforms,
      vertexShader: PICK_VERT,
      fragmentShader: PICK_FRAG,
    });

    const cursors = new Uint32Array(GEOMETRY_FAMILIES.length);
    const buffers = GEOMETRY_FAMILIES.map((name, f) => {
      const n = counts[f];
      return {
        name,
        n,
        color: new Float32Array(n * 3),
        pick: new Float32Array(n * 3),
        scale: new Float32Array(n),
        state: new Float32Array(n),
        matrix: new Float32Array(n * 16),
        nodeIndex: new Uint32Array(n),
      };
    });

    const m = new Matrix4();
    for (let i = 0; i < graph.nodeCount; i++) {
      const f = graph.kind[i];
      const buf = buffers[f];
      const k = cursors[f]++;
      m.makeTranslation(
        graph.positions[i * 3],
        graph.positions[i * 3 + 1],
        graph.positions[i * 3 + 2],
      );
      m.toArray(buf.matrix, k * 16);
      const [cr, cg, cb] = colorToLinearRgb(
        nodeColor(palette, graph.cluster[i], graph.status[i], graph.framework?.[i] ?? 0),
      );
      buf.color[k * 3] = cr;
      buf.color[k * 3 + 1] = cg;
      buf.color[k * 3 + 2] = cb;
      const pick = encodePickId(i);
      buf.pick[k * 3] = pick[0];
      buf.pick[k * 3 + 1] = pick[1];
      buf.pick[k * 3 + 2] = pick[2];
      buf.scale[k] = weightScale(graph.weight[i], minW, maxW);
      buf.nodeIndex[k] = i;
      this.nodeToInstance[i * 2] = f;
      this.nodeToInstance[i * 2 + 1] = k;
    }

    buffers.forEach((buf, f) => {
      const geom = familyGeometry(buf.name, this.lod);
      const mesh = new InstancedMesh(geom, this.material, Math.max(buf.n, 0));
      mesh.frustumCulled = false; // one mesh spans the whole corpus
      mesh.instanceMatrix.array.set(buf.matrix);
      mesh.instanceMatrix.needsUpdate = true;
      geom.setAttribute('aColor', new InstancedBufferAttribute(buf.color, 3));
      geom.setAttribute('aPick', new InstancedBufferAttribute(buf.pick, 3));
      geom.setAttribute('aScale', new InstancedBufferAttribute(buf.scale, 1));
      geom.setAttribute('aState', new InstancedBufferAttribute(buf.state, 1));
      mesh.userData.family = f;
      this.meshes.push(mesh);
      this.instanceToNode.push(buf.nodeIndex);
    });
  }

  addTo(scene) {
    // Families without a single node are not added: today the whole corpus is
    // `law`, so five of the six meshes would be empty draw calls.
    for (const mesh of this.meshes) {
      if (mesh.count > 0) scene.add(mesh);
    }
  }

  setWeightMode(on) {
    this.uniforms.uWeightMix.value = on ? 1 : 0;
  }

  setDimOthers(on) {
    this.uniforms.uDimOthers.value = on ? 1 : 0;
  }

  /**
   * Tell the shader how many world units one device pixel is worth at unit
   * view depth, so the pixel floors mean the same thing on every viewport and
   * every device pixel ratio.
   *
   * @param {number} fovRadians vertical field of view
   * @param {number} drawingBufferHeight height in device pixels
   */
  setPixelScale(fovRadians, drawingBufferHeight) {
    const height = Math.max(1, drawingBufferHeight);
    this.uniforms.uPixelScale.value = (2 * Math.tan(fovRadians / 2)) / height;
  }

  /**
   * Set the render state of one node. O(1), and the upload is limited to the
   * touched slice: marking the attribute dirty without a range re-uploads the
   * whole per-family buffer, which at 100.000 nodes is hundreds of kilobytes
   * per hover frame.
   */
  setState(nodeIndex, state) {
    if (nodeIndex < 0 || nodeIndex >= this.graph.nodeCount) return;
    const f = this.nodeToInstance[nodeIndex * 2];
    const k = this.nodeToInstance[nodeIndex * 2 + 1];
    if (f < 0) return;
    const attr = this.meshes[f].geometry.getAttribute('aState');
    attr.array[k] = state;
    this.markDirty(f, 'aState', k);
  }

  /**
   * Remember which instances changed, per family and per attribute, and upload
   * them in one range at the start of the next frame.
   *
   * Doing this per write instead would either re-upload the whole buffer
   * (hundreds of kilobytes per hover at corpus scale) or leave the update range
   * bookkeeping spread over the callers, where it is easy to get wrong - and a
   * wrong range is a silent one: the array holds the new colour and the screen
   * keeps the old.
   */
  markDirty(family, attribute, index) {
    let perFamily = this.dirty.get(family);
    if (!perFamily) {
      perFamily = new Map();
      this.dirty.set(family, perFamily);
    }
    const range = perFamily.get(attribute);
    if (!range) perFamily.set(attribute, { min: index, max: index });
    else {
      if (index < range.min) range.min = index;
      if (index > range.max) range.max = index;
    }
  }

  /** Push the pending instance writes to the GPU. Called once per frame. */
  flushUpdates() {
    if (this.dirty.size === 0) return 0;
    let flushed = 0;
    for (const [family, perFamily] of this.dirty) {
      const geometry = this.meshes[family]?.geometry;
      if (!geometry) continue;
      for (const [name, range] of perFamily) {
        const attr = geometry.getAttribute(name);
        if (!attr) continue;
        const itemSize = attr.itemSize;
        attr.clearUpdateRanges?.();
        attr.addUpdateRange?.(range.min * itemSize, (range.max - range.min + 1) * itemSize);
        attr.needsUpdate = true;
        flushed++;
      }
    }
    this.dirty.clear();
    return flushed;
  }

  /** Reset every state byte. O(n) but only on an explicit clear. */
  clearStates() {
    for (let f = 0; f < this.meshes.length; f++) {
      const attr = this.meshes[f].geometry.getAttribute('aState');
      attr.array.fill(STATE_NORMAL);
      if (attr.count > 0) this.markDirty(f, 'aState', 0);
      if (attr.count > 0) this.markDirty(f, 'aState', attr.count - 1);
    }
  }

  /**
   * Change the enrichment status of one node and repaint just that instance.
   *
   * This is the hook for the live feed: when the enricher starts on a law the
   * data layer sends one status update, and the colour of one node changes
   * without touching the payload, the layout or any other buffer.
   */
  setStatus(nodeIndex, status) {
    if (nodeIndex < 0 || nodeIndex >= this.graph.nodeCount) return false;
    if (this.graph.status[nodeIndex] === status) return false;
    this.graph.status[nodeIndex] = status;
    const f = this.nodeToInstance[nodeIndex * 2];
    const k = this.nodeToInstance[nodeIndex * 2 + 1];
    if (f < 0) return false;
    const attr = this.meshes[f].geometry.getAttribute('aColor');
    const [r, g, b] = colorToLinearRgb(
      nodeColor(this.palette, this.graph.cluster[nodeIndex], status, this.graph.framework?.[nodeIndex] ?? 0),
    );
    attr.array[k * 3] = r;
    attr.array[k * 3 + 1] = g;
    attr.array[k * 3 + 2] = b;
    this.markDirty(f, 'aColor', k);
    return true;
  }

  /** Depth-cue range in view units; set from the camera distance. */
  setFogRange(near, far) {
    this.uniforms.uFogRange.value.set(near, far);
  }

  updatePalette(palette) {
    this.palette = palette;
    this.uniforms.uBackground.value = new Color(palette.background);
    this.uniforms.uSelection.value = new Color(palette.selection);
    this.uniforms.uFogColor.value = new Color(palette.background);
    const g = this.graph;
    for (let i = 0; i < g.nodeCount; i++) {
      const f = this.nodeToInstance[i * 2];
      const k = this.nodeToInstance[i * 2 + 1];
      if (f < 0) continue;
      const [cr, cg, cb] = colorToLinearRgb(
        nodeColor(palette, g.cluster[i], g.status[i], g.framework?.[i] ?? 0),
      );
      const arr = this.meshes[f].geometry.getAttribute('aColor').array;
      arr[k * 3] = cr;
      arr[k * 3 + 1] = cg;
      arr[k * 3 + 2] = cb;
    }
    for (const mesh of this.meshes) mesh.geometry.getAttribute('aColor').needsUpdate = true;
  }

  useMaterial(which) {
    const material = which === 'pick' ? this.pickMaterial : this.material;
    for (const mesh of this.meshes) mesh.material = material;
  }

  dispose() {
    for (const mesh of this.meshes) {
      mesh.geometry.dispose();
      mesh.removeFromParent();
      mesh.dispose();
    }
    this.material.dispose();
    this.pickMaterial.dispose();
    this.meshes = [];
  }
}
