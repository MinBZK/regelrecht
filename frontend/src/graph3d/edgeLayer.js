/**
 * Edges as a single LineSegments: one draw call for the whole corpus.
 *
 * Two things decide whether this survives at corpus scale, and both are about
 * memory, not draw calls:
 *
 * - `LineSegments` needs two vertices per edge. At five million edges that is
 *   30 million floats for positions alone, 120 MB, and the GPU upload is the
 *   single most expensive moment in the whole view. The build therefore fills
 *   the buffer in one pass with no intermediate objects.
 * - Per-vertex colour doubles that. As long as the graph holds one edge type -
 *   which it does today, everything is `citation` - the colour is a uniform
 *   and the attribute is not allocated at all. The moment a second type shows
 *   up the attribute appears, and it is a Uint8 normalised attribute, a
 *   quarter of the float version.
 *
 * `linewidth` is ignored by every browser on `LineSegments`, so thickness is
 * not available here. It is available on `ThickEdgeLayer` (three's Line2),
 * which builds a quad per segment and costs roughly four times the memory;
 * that is reserved for the selected subgraph, per the design.
 */

import { colorToLinearBytes } from './palette.js';
import {
  BufferAttribute,
  BufferGeometry,
  Color,
  LineBasicMaterial,
  LineSegments,
} from 'three';
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js';
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';

/** True when the graph carries more than one edge type. */
export function needsPerEdgeColor(edgeType) {
  if (!edgeType || edgeType.length === 0) return false;
  const first = edgeType[0];
  for (let i = 1; i < edgeType.length; i++) {
    if (edgeType[i] !== first) return true;
  }
  return false;
}

/**
 * Fill a flat position buffer with one segment per edge.
 * Exported so the packing can be tested without a WebGL context.
 */
export function buildEdgePositions(graph, out) {
  const { edgeCount, edgeSource, edgeTarget, positions } = graph;
  const buf = out ?? new Float32Array(edgeCount * 6);
  for (let e = 0; e < edgeCount; e++) {
    const s = edgeSource[e] * 3;
    const t = edgeTarget[e] * 3;
    const o = e * 6;
    buf[o] = positions[s];
    buf[o + 1] = positions[s + 1];
    buf[o + 2] = positions[s + 2];
    buf[o + 3] = positions[t];
    buf[o + 4] = positions[t + 1];
    buf[o + 5] = positions[t + 2];
  }
  return buf;
}

export function buildEdgeColors(graph, palette, out) {
  const { edgeCount, edgeType } = graph;
  const buf = out ?? new Uint8Array(edgeCount * 6);
  // One conversion per edge type, not per edge: at five million edges the
  // naive version is fifteen million pow() calls and as many throwaway arrays.
  const lut = new Uint8Array(palette.edgeTypes.length * 3);
  palette.edgeTypes.forEach((c, i) => lut.set(colorToLinearBytes(c), i * 3));
  for (let e = 0; e < edgeCount; e++) {
    const t = (edgeType[e] % palette.edgeTypes.length) * 3;
    const r = lut[t];
    const g = lut[t + 1];
    const b = lut[t + 2];
    const o = e * 6;
    buf[o] = r;
    buf[o + 1] = g;
    buf[o + 2] = b;
    buf[o + 3] = r;
    buf[o + 4] = g;
    buf[o + 5] = b;
  }
  return buf;
}

export class EdgeLayer {
  constructor(graph, palette, { opacity = 0.32 } = {}) {
    this.graph = graph;
    this.palette = palette;
    this.perEdgeColor = needsPerEdgeColor(graph.edgeType);

    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new BufferAttribute(buildEdgePositions(graph), 3));
    if (this.perEdgeColor) {
      geometry.setAttribute(
        'color',
        new BufferAttribute(buildEdgeColors(graph, palette), 3, true),
      );
    }
    geometry.boundingSphere = null;

    this.material = new LineBasicMaterial({
      color: this.perEdgeColor ? 0xffffff : palette.edgeTypes[0],
      vertexColors: this.perEdgeColor,
      transparent: true,
      opacity,
      depthWrite: false,
    });
    this.mesh = new LineSegments(geometry, this.material);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = -1; // behind the nodes
  }

  addTo(scene) {
    scene.add(this.mesh);
  }

  setOpacity(value) {
    this.material.opacity = value;
  }

  updatePalette(palette) {
    this.palette = palette;
    if (this.perEdgeColor) {
      const attr = this.mesh.geometry.getAttribute('color');
      buildEdgeColors(this.graph, palette, attr.array);
      attr.needsUpdate = true;
    } else {
      this.material.color = new Color(palette.edgeTypes[0]);
    }
  }

  dispose() {
    this.mesh.geometry.dispose();
    this.material.dispose();
    this.mesh.removeFromParent();
  }
}

/**
 * Thick edges (three's Line2 family) for a small, explicitly chosen subset:
 * the selected node's subgraph, or a highlighted closure. Every segment
 * becomes a quad with per-instance endpoints, so this is instanced geometry
 * and its cost scales with the segment count in both memory and fill rate.
 * Do not point it at the whole graph; that is exactly the limit the design
 * warns about and the benchmark measures.
 */
export class ThickEdgeLayer {
  constructor(palette, { width = 3, color = null, capacity = 4096 } = {}) {
    this.capacity = capacity;
    this.geometry = new LineSegmentsGeometry();
    // Allocate the interleaved buffer once. `setPositions` builds a fresh
    // InstancedInterleavedBuffer every call, and the old one keeps its GPU
    // buffer until the geometry is disposed - which on a per-hover rebuild
    // leaks a buffer per pointer move.
    this.positions = new Float32Array(capacity * 6);
    this.geometry.setPositions(this.positions);
    this.geometry.instanceCount = 0;
    this.material = new LineMaterial({
      color: color ?? palette.selection,
      linewidth: width, // in px, LineMaterial resolves against `resolution`
      transparent: true,
      opacity: 0.95,
      depthTest: true,
    });
    this.mesh = new LineSegments2(this.geometry, this.material);
    this.mesh.frustumCulled = false;
    this.mesh.visible = false;
    this.mesh.renderOrder = 1;
  }

  addTo(scene) {
    scene.add(this.mesh);
  }

  setResolution(width, height) {
    this.material.resolution.set(width, height);
  }

  /**
   * @param {import('./graphSchema.js').PackedGraph} graph
   * @param {Uint32Array|number[]} edgeIndices which edges to draw thick
   */
  setEdges(graph, edgeIndices) {
    const n = Math.min(edgeIndices.length, this.capacity);
    if (n === 0) {
      this.mesh.visible = false;
      this.geometry.instanceCount = 0;
      return 0;
    }
    const pos = this.positions;
    for (let i = 0; i < n; i++) {
      const e = edgeIndices[i];
      const s = graph.edgeSource[e] * 3;
      const t = graph.edgeTarget[e] * 3;
      const o = i * 6;
      pos[o] = graph.positions[s];
      pos[o + 1] = graph.positions[s + 1];
      pos[o + 2] = graph.positions[s + 2];
      pos[o + 3] = graph.positions[t];
      pos[o + 4] = graph.positions[t + 1];
      pos[o + 5] = graph.positions[t + 2];
    }
    const start = this.geometry.getAttribute('instanceStart');
    const buffer = start.data ?? start;
    buffer.array.set(pos.subarray(0, n * 6));
    if (typeof buffer.clearUpdateRanges === 'function') {
      buffer.clearUpdateRanges();
      buffer.addUpdateRange(0, n * 6);
    }
    buffer.needsUpdate = true;
    this.geometry.instanceCount = n;
    this.mesh.visible = true;
    return n;
  }

  clear() {
    this.mesh.visible = false;
    this.geometry.instanceCount = 0;
  }

  dispose() {
    this.geometry.dispose();
    this.material.dispose();
    this.mesh.removeFromParent();
  }
}
