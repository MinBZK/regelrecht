/**
 * The 3D corpus graph: scene, camera, layers, interaction, measurement.
 *
 * Everything that costs time per frame lives here and nowhere else, and the
 * rule it follows is that a frame does three things - update the controls,
 * render, sample the clock. Anything heavier (label selection, neighbour
 * lookup, buffer rewrites) is throttled and happens between frames.
 *
 * The renderer is three.js used directly. A wrapper such as 3d-force-graph
 * gives one mesh per node and one line per edge, which is one draw call per
 * object and dies somewhere around two thousand objects on integrated
 * graphics; the instanced layers here draw the entire corpus in single digits
 * of draw calls.
 */

import {
  Color,
  Frustum,
  Matrix4,
  PerspectiveCamera,
  Scene,
  Vector3,
  WebGLRenderer,
} from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { NodeLayer, STATE_NORMAL, STATE_HIGHLIGHT, STATE_SELECTED } from './nodeLayer.js';
import { EdgeLayer, ThickEdgeLayer } from './edgeLayer.js';
import { LabelLayer, selectLabels } from './labelLayer.js';
import { isEnriched } from './graphSchema.js';
import { buildSdfAtlas } from './sdfAtlas.js';
import { GpuPicker } from './picker.js';
import { FrameStats } from './frameStats.js';
import { readPalette } from './palette.js';

/**
 * Neighbour lists in CSR form: one Uint32Array of targets plus an offset
 * array. Built once, O(E), and it is what makes hover O(degree) instead of
 * O(E). A framework law with thousands of edges is exactly why this cannot be
 * a scan over the edge list per hover.
 */
export function buildAdjacency(graph) {
  const n = graph.nodeCount;
  const counts = new Uint32Array(n + 1);
  for (let e = 0; e < graph.edgeCount; e++) {
    counts[graph.edgeSource[e]]++;
    counts[graph.edgeTarget[e]]++;
  }
  const offsets = new Uint32Array(n + 1);
  let acc = 0;
  for (let i = 0; i < n; i++) {
    offsets[i] = acc;
    acc += counts[i];
  }
  offsets[n] = acc;
  const cursor = offsets.slice(0, n);
  const neighbours = new Uint32Array(acc);
  const edgeOf = new Uint32Array(acc);
  for (let e = 0; e < graph.edgeCount; e++) {
    const s = graph.edgeSource[e];
    const t = graph.edgeTarget[e];
    neighbours[cursor[s]] = t;
    edgeOf[cursor[s]] = e;
    cursor[s]++;
    neighbours[cursor[t]] = s;
    edgeOf[cursor[t]] = e;
    cursor[t]++;
  }
  return { offsets, neighbours, edgeOf };
}

/**
 * The order labels are handed out in: everything enriched first, by weight,
 * then the grey remainder by weight.
 *
 * That follows from the colour rule. In a grey corpus with a handful of
 * coloured laws, the coloured ones are what the map is about, so they get a
 * name before a heavier grey neighbour does.
 */
export function labelOrder(graph) {
  const enriched = [];
  const rest = [];
  for (let i = 0; i < graph.nodeCount; i++) {
    (isEnriched(graph.status[i]) ? enriched : rest).push(i);
  }
  const byWeight = (a, b) => graph.weight[b] - graph.weight[a];
  enriched.sort(byWeight);
  rest.sort(byWeight);
  return Int32Array.from(enriched.concat(rest));
}

/**
 * Bounding box of the layout, plus the node size that fits it.
 *
 * The payload's coordinates are whatever the layout produced - the real corpus
 * spans a few hundred units with the framework laws thrown far out - so a fixed
 * node radius is either invisible or a solid blob. Sizing off the mean spacing
 * (`2R / cbrt(N)`) keeps the density of the picture constant whatever the
 * layout's units happen to be.
 */
export function graphExtent(graph) {
  let minX = Infinity;
  let minY = Infinity;
  let minZ = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  let maxZ = -Infinity;
  const p = graph.positions;
  for (let i = 0; i < graph.nodeCount; i++) {
    const x = p[i * 3];
    const y = p[i * 3 + 1];
    const z = p[i * 3 + 2];
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (z < minZ) minZ = z;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
    if (z > maxZ) maxZ = z;
  }
  if (!Number.isFinite(minX)) {
    minX = minY = minZ = -1;
    maxX = maxY = maxZ = 1;
  }
  const radius = Math.max(1e-3, Math.max(maxX - minX, maxY - minY, maxZ - minZ) / 2);
  const spacing = (2 * radius) / Math.max(1, Math.cbrt(graph.nodeCount));
  return {
    center: [(minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2],
    radius,
    // A quarter of the mean spacing: dense enough to read as a field, open
    // enough that a node does not hide its neighbours.
    baseSize: Math.max(1e-4, spacing * 0.25),
  };
}

const DEFAULT_OPTIONS = {
  labelBudget: 400,
  labelInterval: 120,
  maxNeighbourHighlight: 400,
  // Thick edges are quads, and on a software rasteriser they cost a fixed
  // ~220 ms per frame from four thousand segments upwards - measured, not
  // guessed. They are therefore only for the highlighted subgraph, and the
  // limit is low on purpose: the base graph stays on thin LineSegments, which
  // costs a thousandth of that.
  thickEdgeLimit: 1024,
  reducedMotion: false,
  weightMode: true,
  showLabels: true,
};

export class GraphScene {
  /**
   * @param {HTMLCanvasElement} canvas
   * @param {import('./graphSchema.js').PackedGraph} graph
   * @param {object} [options]
   */
  constructor(canvas, graph, options = {}) {
    this.canvas = canvas;
    this.graph = graph;
    this.options = { ...DEFAULT_OPTIONS, ...options };
    this.palette = options.palette ?? readPalette(options.styleRoot);

    this.renderer = new WebGLRenderer({
      canvas,
      antialias: graph.nodeCount <= 60000,
      powerPreference: 'high-performance',
    });
    this.renderer.setPixelRatio(Math.min(globalThis.devicePixelRatio || 1, 2));
    this.renderer.setClearColor(new Color(this.palette.background), 1);

    // No lights in the scene: both layers are ShaderMaterials that do their
    // own shading, so a light would be an object three has to collect and
    // upload every frame for nothing.
    this.scene = new Scene();

    this.camera = new PerspectiveCamera(55, 1, 0.5, 100000);

    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = !this.options.reducedMotion;
    this.controls.dampingFactor = 0.12;
    this.controls.rotateSpeed = 0.6;

    this.extent = graphExtent(graph);
    this.nodes = new NodeLayer(graph, this.palette, {
      weightMode: this.options.weightMode,
      baseSize: this.options.baseSize ?? this.extent.baseSize,
    });
    this.nodes.addTo(this.scene);

    // Line strength against edge count: at thirty thousand edges 0.3 reads as
    // a web, at a million the same value is a solid fill. The exponent keeps a
    // single edge visible while a dense bundle stays a bundle.
    const edgeOpacity = Math.max(
      0.06,
      Math.min(0.4, 0.4 * Math.pow(30000 / Math.max(graph.edgeCount, 1), 0.35)),
    );
    this.edges = new EdgeLayer(graph, this.palette, { opacity: edgeOpacity });
    this.edges.mesh.userData.pickable = false;
    this.edges.addTo(this.scene);

    this.thickEdges = new ThickEdgeLayer(this.palette, {
      capacity: this.options.thickEdgeLimit,
    });
    this.thickEdges.mesh.userData.pickable = false;
    this.thickEdges.addTo(this.scene);

    this.labelsUnavailable = false;
    if (this.options.showLabels) {
      this.atlas = options.atlas ?? buildSdfAtlas();
      if (this.atlas.usable === false) this.labelsUnavailable = true;
    }
    if (this.options.showLabels && !this.labelsUnavailable) {
      this.labels = new LabelLayer(this.atlas, this.palette, {
        budget: this.options.labelBudget,
      });
      this.labels.mesh.userData.pickable = false;
      this.labels.addTo(this.scene);
      this.weightOrder = labelOrder(graph);
    }

    this.picker = new GpuPicker(this.renderer, this.scene, this.camera, this.nodes);
    this.stats = new FrameStats(180);
    this.pickStats = new FrameStats(60);

    this.adjacency = null; // built lazily: O(E) and only needed on first hover
    this.hovered = -1;
    this.selected = -1;
    this.highlighted = [];
    this.frustum = new Frustum();
    this.projScreen = new Matrix4();
    this.tmpVec = new Vector3();
    this.labelsDirty = true;
    this.labelsEnabled = true;
    this.lastLabelUpdate = 0;
    this.labelInterval = this.options.labelInterval ?? 120;
    this.running = false;

    this.resize();
    this.fitAll();
  }

  /** Canvas size follows its CSS box; call on resize and on DPR changes. */
  resize() {
    // The device pixel ratio changes when the window moves to another screen,
    // so it belongs here and not only in the constructor.
    this.renderer.setPixelRatio(Math.min(globalThis.devicePixelRatio || 1, 2));
    const rect = this.canvas.getBoundingClientRect();
    const width = Math.max(1, Math.round(rect.width || this.canvas.width));
    const height = Math.max(1, Math.round(rect.height || this.canvas.height));
    this.width = width;
    this.height = height;
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.thickEdges.setResolution(width, height);
    if (this.labels) this.labels.setResolution(width, height);
    this.labelsDirty = true;
  }

  /** Frame the whole corpus: the "hele graaf in beeld" action. */
  fitAll() {
    const { center, radius } = this.extent;
    const [cx, cy, cz] = center;
    const dist = (radius * 1.6) / Math.tan((this.camera.fov * Math.PI) / 360);
    this.controls.target.set(cx, cy, cz);
    this.camera.position.set(cx + dist * 0.4, cy + dist * 0.35, cz + dist * 0.85);
    this.camera.near = Math.max(radius / 5000, dist / 5000);
    this.camera.far = dist * 20;
    this.camera.updateProjectionMatrix();
    this.controls.update();
    // The depth cue is scaled to the graph, not to a fixed distance: the front
    // of the cloud stays crisp and the far side sinks into the background.
    // The near plane of the cue sits just in front of the graph's centre, so
    // the nearest third stays at full strength.
    this.nodes.setFogRange(Math.max(1, dist + radius * 0.1), dist + radius * 2.2);
    this.labelsDirty = true;
  }

  /** Fly (or jump, under reduced motion) to one node and select it. */
  focusNode(nodeIndex) {
    if (nodeIndex < 0) return;
    const p = this.graph.positions;
    const target = new Vector3(p[nodeIndex * 3], p[nodeIndex * 3 + 1], p[nodeIndex * 3 + 2]);
    const dir = this.camera.position.clone().sub(this.controls.target).normalize();
    const dist = Math.max(12, this.controls.target.distanceTo(this.camera.position) * 0.25);
    this.controls.target.copy(target);
    this.camera.position.copy(target).addScaledVector(dir, dist);
    this.controls.update();
    this.select(nodeIndex);
    this.labelsDirty = true;
  }

  ensureAdjacency() {
    if (!this.adjacency) this.adjacency = buildAdjacency(this.graph);
    return this.adjacency;
  }

  /** Neighbours of a node, capped so a framework-law star cannot stall a hover. */
  neighboursOf(nodeIndex, cap = this.options.maxNeighbourHighlight) {
    const { offsets, neighbours, edgeOf } = this.ensureAdjacency();
    const start = offsets[nodeIndex];
    const end = offsets[nodeIndex + 1];
    const n = Math.min(end - start, cap);
    const nodes = new Uint32Array(n);
    const edges = new Uint32Array(n);
    for (let k = 0; k < n; k++) {
      nodes[k] = neighbours[start + k];
      edges[k] = edgeOf[start + k];
    }
    return { nodes, edges, degree: end - start, truncated: end - start > n };
  }

  clearHighlight() {
    for (const i of this.highlighted) this.nodes.setState(i, STATE_NORMAL);
    this.highlighted = [];
    this.nodes.setDimOthers(false);
    this.thickEdges.clear();
    if (this.selected >= 0) this.nodes.setState(this.selected, STATE_SELECTED);
  }

  /**
   * Highlight a node and its neighbourhood; everything else dims. O(degree),
   * not O(n): the dim is a uniform, only the highlighted nodes are written.
   */
  highlight(nodeIndex) {
    this.clearHighlight();
    if (nodeIndex < 0) return null;
    const { nodes, edges, degree, truncated } = this.neighboursOf(nodeIndex);
    this.highlighted = [nodeIndex, ...nodes];
    this.nodes.setState(nodeIndex, STATE_SELECTED);
    for (const i of nodes) this.nodes.setState(i, STATE_HIGHLIGHT);
    this.nodes.setDimOthers(true);
    const thick = edges.length <= this.options.thickEdgeLimit ? edges : edges.slice(0, this.options.thickEdgeLimit);
    this.thickEdges.setEdges(this.graph, thick);
    this.labelsDirty = true;
    return { degree, truncated, shown: nodes.length };
  }

  select(nodeIndex) {
    this.selected = nodeIndex;
    return this.highlight(nodeIndex);
  }

  hover(nodeIndex) {
    if (nodeIndex === this.hovered) return null;
    this.hovered = nodeIndex;
    if (nodeIndex < 0) {
      if (this.selected >= 0) return this.highlight(this.selected);
      this.clearHighlight();
      return null;
    }
    return this.highlight(nodeIndex);
  }

  /** Node under a CSS pixel, through the GPU id pass. */
  pickAt(x, y) {
    const t0 = performance.now();
    const id = this.picker.pick(x, y, this.width, this.height);
    this.pickStats.push(performance.now() - t0);
    return id;
  }

  /**
   * Recompute which labels are drawn. Throttled: it walks the weight order
   * until the budget is full, which is cheap when the camera looks at a dense
   * region and O(n) in the worst case (a nearly empty view), so it must not
   * run every frame.
   */
  updateLabels(now, force = false) {
    if (!this.labels || this.labelsEnabled === false) return;
    // Even a dirty view rebuilds at most `labelInterval` apart: the LOD pass
    // walks the weight order and rewrites the glyph buffers, which is far too
    // much to do at frame rate while the camera is moving. Eight times a
    // second is fast enough that labels never feel stuck to the old view.
    if (!force && (now - this.lastLabelUpdate < this.labelInterval || !this.labelsDirty)) return;
    this.lastLabelUpdate = now;
    this.labelsDirty = false;

    // `matrixWorldInverse` is refreshed by the renderer, which has not run
    // yet this frame; without this the labels are placed against the previous
    // camera position, one frame behind everything else.
    this.camera.updateMatrixWorld();
    this.camera.matrixWorldInverse.copy(this.camera.matrixWorld).invert();
    this.projScreen.multiplyMatrices(
      this.camera.projectionMatrix,
      this.camera.matrixWorldInverse,
    );
    this.frustum.setFromProjectionMatrix(this.projScreen);
    const p = this.graph.positions;
    const vec = this.tmpVec;

    // Occupancy grid in screen space. Three hundred labels on the real corpus
    // land on top of each other because the layout is dense in the middle, and
    // overlapping text is worse than no text: a label that cannot be read costs
    // fill rate and gives nothing. A label claims the cells its box covers, and
    // the next candidate for those cells goes unlabelled.
    const cellW = 110;
    const cellH = 16;
    const cols = Math.max(1, Math.ceil(this.width / cellW));
    const rows = Math.max(1, Math.ceil(this.height / cellH));
    if (!this.labelGrid || this.labelGrid.length !== cols * rows) {
      this.labelGrid = new Uint8Array(cols * rows);
    }
    const grid = this.labelGrid;
    grid.fill(0);

    const claim = (i, occupy) => {
      vec.set(p[i * 3], p[i * 3 + 1], p[i * 3 + 2]);
      if (!this.frustum.containsPoint(vec)) return false;
      vec.applyMatrix4(this.projScreen);
      const sx = (vec.x * 0.5 + 0.5) * this.width;
      const sy = (0.5 - vec.y * 0.5) * this.height;
      const col = Math.floor(sx / cellW);
      const row = Math.floor(sy / cellH);
      if (col < 0 || row < 0 || col >= cols || row >= rows) return false;
      // A label is wider than one cell, so it claims two: one cell wide would
      // let two labels sit half a word apart and still both be drawn.
      const cell = row * cols + col;
      const right = col + 1 < cols ? cell + 1 : cell;
      if (grid[cell] || grid[right]) return false;
      if (occupy) {
        grid[cell] = 1;
        grid[right] = 1;
      }
      return true;
    };

    const pinned = [];
    if (this.selected >= 0) pinned.push(this.selected);
    if (this.hovered >= 0) pinned.push(this.hovered);
    for (const i of pinned) claim(i, true);

    const chosen = selectLabels(
      this.weightOrder,
      (i) => claim(i, true),
      this.options.labelBudget,
      pinned,
      {
        // Zoomed in on a corner of a 100.000-node corpus almost nothing passes
        // the frustum test, and an uncapped search walks the entire weight
        // order to fill a budget it can never fill. The cap bounds that worst
        // case; with the occupancy grid it is also what stops the search once
        // the screen is full.
        maxScanned: Math.max(this.options.labelBudget * 20, 4000),
      },
    );
    this.labels.setLabels(this.graph, chosen);
    this.labelCount = chosen.length;
  }

  /** Turn labels off entirely: hides the mesh and stops the LOD pass. */
  setLabelsEnabled(on) {
    this.labelsEnabled = on;
    if (this.labels) this.labels.mesh.visible = on;
    if (on) this.labelsDirty = true;
  }

  /**
   * Apply enrichment-status updates from the data layer.
   *
   * The payload is not rebuilt for this and the layout does not move: a status
   * change repaints one instance. That is what makes a live view of a running
   * enricher possible later without re-fetching the graph.
   *
   * @param {Iterable<[number, number]>} updates pairs of [nodeIndex, status]
   * @returns {number} how many nodes actually changed
   */
  applyStatusUpdates(updates) {
    let changed = 0;
    for (const [index, status] of updates) {
      if (this.nodes.setStatus(index, status)) changed++;
    }
    if (changed > 0 && this.labels) {
      // A newly enriched law should get its name; the label order is by
      // enrichment first, so it has to be rebuilt.
      this.weightOrder = labelOrder(this.graph);
      this.labelsDirty = true;
    }
    return changed;
  }

  /** Same, keyed by the payload's stable node ids. */
  applyStatusUpdatesById(updatesById) {
    if (!this.idIndex && this.graph.ids) {
      this.idIndex = new Map(this.graph.ids.map((id, i) => [id, i]));
    }
    const pairs = [];
    for (const [id, status] of updatesById) {
      const i = this.idIndex?.get(id);
      if (i !== undefined) pairs.push([i, status]);
    }
    return this.applyStatusUpdates(pairs);
  }

  /**
   * Whether a node offers more than its name. Grey means only harvested: you
   * can point at it and read what it is called, but there is no article level
   * and no marking panel behind it, because nothing has been made yet.
   */
  isInteractive(nodeIndex) {
    if (nodeIndex < 0 || nodeIndex >= this.graph.nodeCount) return false;
    return isEnriched(this.graph.status[nodeIndex]);
  }

  setLabelBudget(budget) {
    this.options.labelBudget = Math.min(budget, this.labels ? this.labels.budget : budget);
    this.labelsDirty = true;
  }

  setWeightMode(on) {
    this.options.weightMode = on;
    this.nodes.setWeightMode(on);
  }

  updatePalette(palette) {
    this.palette = palette;
    this.renderer.setClearColor(new Color(palette.background), 1);
    this.nodes.updatePalette(palette);
    this.edges.updatePalette(palette);
    if (this.labels) this.labels.updatePalette(palette);
  }

  /** One frame. Returns the time the render itself took, in ms. */
  renderFrame(now = performance.now()) {
    if (this.controls.enableDamping) this.controls.update();
    if (this.camera.matrixWorldNeedsUpdate || this.controlsMoved()) this.labelsDirty = true;
    this.updateLabels(now);
    // One upload per frame for everything hover, selection and status changed
    // since the last one.
    this.nodes.flushUpdates();
    const t0 = performance.now();
    this.renderer.render(this.scene, this.camera);
    const dt = performance.now() - t0;
    this.stats.mark(now);
    return dt;
  }

  controlsMoved() {
    const p = this.camera.position;
    const moved =
      !this.lastCam ||
      Math.abs(this.lastCam.x - p.x) > 1e-3 ||
      Math.abs(this.lastCam.y - p.y) > 1e-3 ||
      Math.abs(this.lastCam.z - p.z) > 1e-3;
    if (moved) this.lastCam = { x: p.x, y: p.y, z: p.z };
    return moved;
  }

  start() {
    if (this.running) return;
    this.running = true;
    const loop = () => {
      if (!this.running) return;
      this.frameHandle = requestAnimationFrame(loop);
      this.renderFrame(performance.now());
    };
    this.frameHandle = requestAnimationFrame(loop);
  }

  stop() {
    this.running = false;
    if (this.frameHandle) cancelAnimationFrame(this.frameHandle);
    this.frameHandle = null;
  }

  dispose() {
    this.stop();
    this.controls.dispose();
    this.picker.dispose();
    this.nodes.dispose();
    this.edges.dispose();
    this.thickEdges.dispose();
    if (this.labels) this.labels.dispose();
    this.renderer.dispose();
    // dispose() frees three's own objects but not the WebGL context itself;
    // without this a handful of rebuilds exhausts the browser's context limit
    // and the oldest canvas goes black.
    this.renderer.forceContextLoss?.();
  }
}
