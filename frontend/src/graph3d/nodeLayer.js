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

  varying vec3 vColor;
  varying vec3 vNormalW;
  varying float vState;
  varying float vDepth;

  void main() {
    float s = uBaseSize * mix(1.0, aScale, uWeightMix);
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
  varying vec3 vPick;
  void main() {
    float s = uBaseSize * mix(1.0, aScale, uWeightMix) * uPickInflate;
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

/**
 * Weight -> radius. Logarithmic, and hard-capped at 4:1 between the largest
 * and the smallest node so a heavyweight never hides its neighbours.
 */
export function weightScale(weight, minWeight, maxWeight) {
  const lo = Math.log(1 + Math.max(0, minWeight));
  const hi = Math.log(1 + Math.max(0, maxWeight));
  if (!(hi > lo)) return 1;
  const t = (Math.log(1 + Math.max(0, weight)) - lo) / (hi - lo);
  return 1 + 3 * Math.min(1, Math.max(0, t));
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
