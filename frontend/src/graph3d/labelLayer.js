/**
 * Labels as instanced SDF glyph quads.
 *
 * One instance per glyph, one draw call for every label on screen. The buffers
 * are allocated once at the label budget (budget x max glyphs per label) and
 * rewritten in place when the visible selection changes; nothing is allocated
 * per frame.
 *
 * The budget is the whole point. Every label costs glyphs, and glyphs cost
 * fill rate, so the layer never draws more than `budget` labels: the visible
 * nodes with the highest weight win, everything else goes unlabelled until you
 * move closer. Selection and hover jump the queue, because a label you asked
 * for must never lose to a heavier neighbour.
 */

import {
  BufferAttribute,
  ClampToEdgeWrapping,
  DataTexture,
  LinearFilter,
  DoubleSide,
  InstancedBufferAttribute,
  InstancedBufferGeometry,
  Color,
  Mesh,
  RedFormat,
  ShaderMaterial,
  Vector2,
} from 'three';
import { layoutLabel } from './sdfAtlas.js';

const LABEL_VERT = /* glsl */ `
  attribute vec3 aAnchor;
  attribute vec2 aOffset;
  attribute vec2 aSize;
  attribute vec4 aUv;
  attribute float aAlpha;

  uniform vec2 uResolution;

  varying vec2 vUv;
  varying float vAlpha;

  void main() {
    vec4 clip = projectionMatrix * modelViewMatrix * vec4(aAnchor, 1.0);
    // Screen-space offset: labels keep their pixel size at any distance, so
    // they stay readable when the camera pulls back over the whole corpus.
    vec2 px = (aOffset + (position.xy + 0.5) * aSize) / uResolution * 2.0 * clip.w;
    clip.xy += px;
    vUv = mix(aUv.xy, aUv.zw, position.xy + 0.5);
    vAlpha = aAlpha;
    gl_Position = clip;
  }
`;

const LABEL_FRAG = /* glsl */ `
  precision mediump float;

  uniform sampler2D uAtlas;
  uniform vec3 uColor;
  uniform vec3 uHalo;

  varying vec2 vUv;
  varying float vAlpha;

  void main() {
    float d = texture2D(uAtlas, vUv).r;
    float w = fwidth(d);
    float ink = smoothstep(0.5 - w, 0.5 + w, d);
    // A one-pixel halo in the background colour keeps the text legible on top
    // of a dense node cloud without a second draw call.
    float halo = smoothstep(0.5 - w - 0.18, 0.5 + w, d);
    if (halo < 0.01) discard;
    vec3 c = mix(uHalo, uColor, ink);
    gl_FragColor = vec4(c, halo * vAlpha);
    #include <colorspace_fragment>
  }
`;

/**
 * Pick the labels to draw: highest weight first, limited by the budget, with
 * pinned nodes (selection, hover) always included. Pure, so the LOD rule is
 * testable without a renderer.
 *
 * @param {Int32Array|number[]} weightOrder node indices, heaviest first
 * @param {(i: number) => boolean} isVisible frustum / filter test
 * @param {number} budget
 * @param {number[]} [pinned]
 * @param {{maxScanned?: number}} [opts] cap on how far down the weight order to look
 * @returns {number[]}
 */
export function selectLabels(weightOrder, isVisible, budget, pinned = [], { maxScanned = Infinity } = {}) {
  const out = [];
  const seen = new Set();
  for (const p of pinned) {
    if (p >= 0 && !seen.has(p)) {
      seen.add(p);
      out.push(p);
      if (out.length >= budget) return out;
    }
  }
  const limit = Math.min(weightOrder.length, maxScanned);
  for (let k = 0; k < limit && out.length < budget; k++) {
    const i = weightOrder[k];
    if (seen.has(i) || !isVisible(i)) continue;
    seen.add(i);
    out.push(i);
  }
  return out;
}

/** Node indices sorted by descending weight. Computed once per graph. */
export function weightOrder(weight) {
  const order = new Int32Array(weight.length);
  for (let i = 0; i < weight.length; i++) order[i] = i;
  const arr = Array.from(order);
  arr.sort((a, b) => weight[b] - weight[a]);
  return Int32Array.from(arr);
}

function markRange(attribute, instanceCount) {
  const count = instanceCount * attribute.itemSize;
  if (typeof attribute.clearUpdateRanges === 'function') {
    attribute.clearUpdateRanges();
    attribute.addUpdateRange(0, count);
  } else if (attribute.updateRange) {
    attribute.updateRange.offset = 0;
    attribute.updateRange.count = count;
  }
  attribute.needsUpdate = true;
}

export class LabelLayer {
  /**
   * @param {object} atlas result of buildSdfAtlas()
   * @param {object} palette
   * @param {object} [opts]
   */
  constructor(atlas, palette, { budget = 400, maxGlyphs = 28, pixelSize = 13 } = {}) {
    this.atlas = atlas;
    this.budget = budget;
    this.maxGlyphs = maxGlyphs;
    this.pixelSize = pixelSize;
    this.capacity = budget * maxGlyphs;

    const texture = new DataTexture(
      atlas.texture.data,
      atlas.texture.width,
      atlas.texture.height,
      RedFormat,
    );
    texture.needsUpdate = true;
    texture.generateMipmaps = false;
    // Linear filtering is not cosmetic here: the fragment shader takes fwidth
    // of the sampled distance, and a nearest-sampled field gives it stair steps
    // instead of a gradient - which is the entire point of an SDF gone.
    texture.minFilter = LinearFilter;
    texture.magFilter = LinearFilter;
    texture.wrapS = ClampToEdgeWrapping;
    texture.wrapT = ClampToEdgeWrapping;
    this.texture = texture;

    const geometry = new InstancedBufferGeometry();
    // Unit quad centred on the origin: the shared, non-instanced geometry that
    // every glyph instance is placed against in screen space.
    geometry.setAttribute(
      'position',
      new BufferAttribute(
        new Float32Array([-0.5, -0.5, 0, 0.5, -0.5, 0, 0.5, 0.5, 0, -0.5, 0.5, 0]),
        3,
      ),
    );
    geometry.setIndex([0, 1, 2, 0, 2, 3]);

    this.aAnchor = new InstancedBufferAttribute(new Float32Array(this.capacity * 3), 3);
    this.aOffset = new InstancedBufferAttribute(new Float32Array(this.capacity * 2), 2);
    this.aSize = new InstancedBufferAttribute(new Float32Array(this.capacity * 2), 2);
    this.aUv = new InstancedBufferAttribute(new Float32Array(this.capacity * 4), 4);
    this.aAlpha = new InstancedBufferAttribute(new Float32Array(this.capacity), 1);
    geometry.setAttribute('aAnchor', this.aAnchor);
    geometry.setAttribute('aOffset', this.aOffset);
    geometry.setAttribute('aSize', this.aSize);
    geometry.setAttribute('aUv', this.aUv);
    geometry.setAttribute('aAlpha', this.aAlpha);
    geometry.instanceCount = 0;
    this.geometry = geometry;

    this.material = new ShaderMaterial({
      uniforms: {
        uAtlas: { value: texture },
        uResolution: { value: new Vector2(1, 1) },
        uColor: { value: new Color(palette.ink) },
        uHalo: { value: new Color(palette.background) },
      },
      vertexShader: LABEL_VERT,
      fragmentShader: LABEL_FRAG,
      transparent: true,
      depthTest: false,
      depthWrite: false,
      side: DoubleSide,
    });

    this.mesh = new Mesh(geometry, this.material);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = 2;
  }

  addTo(scene) {
    scene.add(this.mesh);
  }

  setResolution(width, height) {
    this.material.uniforms.uResolution.value.set(width, height);
  }

  updatePalette(palette) {
    this.material.uniforms.uColor.value = new Color(palette.ink);
    this.material.uniforms.uHalo.value = new Color(palette.background);
  }

  /**
   * Rewrite the glyph buffers for a set of node indices.
   * @param {import('./graphSchema.js').PackedGraph} graph
   * @param {number[]} nodeIndices
   */
  setLabels(graph, nodeIndices) {
    const glyphs = this.atlas.glyphs;
    const size = this.pixelSize;
    const anchor = this.aAnchor.array;
    const offset = this.aOffset.array;
    const sizeArr = this.aSize.array;
    const uv = this.aUv.array;
    const alpha = this.aAlpha.array;
    let w = 0;

    for (const i of nodeIndices) {
      const text = graph.labels ? graph.labels[i] : String(i);
      if (!text) continue;
      const { quads, width } = layoutLabel(text, glyphs, this.maxGlyphs * 0.62, this.maxGlyphs);
      const startX = -(width * size) / 2;
      for (const q of quads) {
        if (w >= this.capacity) break;
        anchor[w * 3] = graph.positions[i * 3];
        anchor[w * 3 + 1] = graph.positions[i * 3 + 1];
        anchor[w * 3 + 2] = graph.positions[i * 3 + 2];
        offset[w * 2] = startX + (q.x + q.glyph.bearingX) * size;
        offset[w * 2 + 1] = size * 1.1 - q.glyph.bearingY * size;
        sizeArr[w * 2] = q.glyph.w * size;
        sizeArr[w * 2 + 1] = q.glyph.h * size;
        uv[w * 4] = q.glyph.u0;
        uv[w * 4 + 1] = q.glyph.v1;
        uv[w * 4 + 2] = q.glyph.u1;
        uv[w * 4 + 3] = q.glyph.v0;
        alpha[w] = 1;
        w++;
      }
      if (w >= this.capacity) break;
    }

    // Upload only the glyphs actually written. Marking the attribute dirty
    // re-uploads the whole buffer by default, and the whole buffer is the
    // label budget times the maximum glyph count - megabytes per rebuild at a
    // budget of a few thousand, which is where the frame time went before
    // this range was set.
    markRange(this.aAnchor, w);
    markRange(this.aOffset, w);
    markRange(this.aSize, w);
    markRange(this.aUv, w);
    markRange(this.aAlpha, w);
    this.geometry.instanceCount = w;
    this.glyphCount = w;
    return w;
  }

  dispose() {
    this.geometry.dispose();
    this.material.dispose();
    this.texture.dispose();
    this.mesh.removeFromParent();
  }
}
