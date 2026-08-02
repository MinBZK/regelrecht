/**
 * Measurement harness for the 3D corpus graph.
 *
 * Loaded by `bench-graph3d.html` and driven from `scripts/bench-graph3d.mjs`.
 * It exists because "it feels smooth" is not a number: every claim about where
 * this renderer falls over - label budget, edge count, star-shaped hubs - is a
 * frame-time distribution measured here, at a stated size, on a stated device.
 *
 * The camera orbits during the measurement. A static camera measures a warm
 * pipeline with perfect temporal coherence and flatters the result.
 */

import { generateCorpusGraph } from './generateCorpusGraph.js';
import { loadRrgraph } from './rrgraph.js';
import { GraphScene } from './GraphScene.js';
import { buildSdfAtlas } from './sdfAtlas.js';
import { readPalette } from './palette.js';
import { FrameStats } from './frameStats.js';
import { ThickEdgeLayer } from './edgeLayer.js';

let scene = null;
let atlas = null;
let palette = null;

function ensureShared() {
  if (!atlas) atlas = buildSdfAtlas();
  if (!palette) palette = readPalette();
  return { atlas, palette };
}

function percentile(sorted, q) {
  if (sorted.length === 0) return 0;
  return sorted[Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1)))];
}

/**
 * Run one measurement.
 *
 * @param {object} cfg
 * @param {number} cfg.nodes
 * @param {number} cfg.edges
 * @param {number} [cfg.frames]        frames to measure after warm-up
 * @param {number} [cfg.labelBudget]
 * @param {boolean} [cfg.labels]
 * @param {number} [cfg.thickEdges]    measure Line2 with this many segments
 * @param {number} [cfg.hubs]
 */
export async function runCase(cfg) {
  const {
    nodes,
    edges,
    frames = 120,
    labelBudget = 400,
    labels = true,
    thickEdges = 0,
    hubs = 6,
    seed = 7,
    deadlineMs = 90000,
  } = cfg;

  const canvas = document.getElementById('graph-canvas');
  if (scene) {
    scene.dispose();
    scene = null;
  }
  const shared = ensureShared();

  const t0 = performance.now();
  // `file` measures the real payload end to end: fetch, decode and the parent
  // offsetting of article coordinates, which is the path a user actually pays.
  const graph = cfg.file
    ? await loadRrgraph(cfg.file, { lawLevelOnly: cfg.lawLevelOnly !== false, labels })
    : generateCorpusGraph({ nodeCount: nodes, edgeCount: edges, hubs, seed, labels });
  const generateMs = performance.now() - t0;

  const t1 = performance.now();
  scene = new GraphScene(canvas, graph, {
    palette: shared.palette,
    atlas: shared.atlas,
    labelBudget,
    showLabels: labels,
    reducedMotion: true, // no damping: the benchmark drives the camera itself
  });
  const buildMs = performance.now() - t1;

  let thickMs = 0;
  if (thickEdges > 0) {
    const t = performance.now();
    // Capacity has to match the case: the layer refuses to draw more segments
    // than it allocated, and a silently capped measurement would report the
    // cap's frame time instead of the case's.
    const layer = new ThickEdgeLayer(shared.palette, { width: 3, capacity: thickEdges });
    layer.setResolution(scene.width, scene.height);
    const idx = new Uint32Array(Math.min(thickEdges, graph.edgeCount));
    for (let i = 0; i < idx.length; i++) idx[i] = i;
    layer.setEdges(graph, idx);
    layer.addTo(scene.scene);
    layer.mesh.userData.pickable = false;
    scene.benchThickLayer = layer;
    thickMs = performance.now() - t;
  }

  // Warm-up: shader compilation and the first buffer upload are one-off costs
  // and would otherwise land in the tail of the distribution.
  for (let i = 0; i < 12; i++) {
    scene.renderFrame(performance.now());
    await nextFrame();
  }

  // Two clocks, because they measure different things and only reporting the
  // first is how a renderer gets called fast when it is not.
  //
  // `frameP50` is the submission cost: what the main thread spends before it
  // returns. On a GPU-backed pipeline that is close to the truth; on a software
  // rasteriser, which finishes the work on other threads, it is far too
  // flattering.
  //
  // `wallPerFrame` is the honest one: render the whole block, then block on
  // `finish()` and divide. Nothing can hide behind a queue there.
  const stats = new FrameStats(Math.max(frames, 16));
  const frameSamples = [];
  const gl = scene.renderer.getContext();
  gl.finish();
  const blockStart = performance.now();
  const radius = scene.camera.position.distanceTo(scene.controls.target);
  const centre = scene.controls.target.clone();
  let last = performance.now();
  const deadline = last + deadlineMs;
  let aborted = false;
  for (let i = 0; i < frames; i++) {
    if (performance.now() > deadline) {
      // A case that cannot finish its frames inside the deadline has already
      // answered the question it was asked: this size does not render here.
      aborted = true;
      break;
    }
    const a = (i / frames) * Math.PI * 0.6;
    scene.camera.position.set(
      centre.x + Math.cos(a) * radius * 0.9,
      centre.y + radius * 0.35,
      centre.z + Math.sin(a) * radius * 0.9,
    );
    scene.camera.lookAt(centre);
    scene.labelsDirty = true;
    await nextFrame();
    const now = performance.now();
    scene.renderFrame(now);
    frameSamples.push(now - last);
    stats.push(now - last);
    last = now;
  }

  gl.finish();
  const wallPerFrame = (performance.now() - blockStart) / Math.max(1, frameSamples.length);

  // Let the pipeline drain first. `readPixels` blocks until everything queued
  // has been drawn, so a pick measured straight after an unthrottled burst of
  // frames reports the backlog rather than the pick: on this software
  // rasteriser that is the difference between 0,1 and 90 seconds. Picking in a
  // real session always happens on a quiet pipeline, so that is what is
  // measured here.

  // Picking: a handful of samples in the middle of the viewport, with a hard
  // time budget. On a software rasteriser one id pass over 200.000 instances
  // takes seconds, and twenty of those would dominate the whole run without
  // saying anything the first few have not already said.
  // Warm the id pass first. The first pick after a scene is built pays for the
  // pick program's compilation and the render target's allocation, and on this
  // software rasteriser that one-off is measured in tens of seconds while the
  // steady state is a tenth of a second. Hovering is steady state, so that is
  // what the number has to describe.
  scene.pickAt(scene.width / 2, scene.height / 2);
  scene.pickAt(scene.width / 2 + 1, scene.height / 2);

  const pickSamples = [];
  const pickDeadline = performance.now() + 6000;
  for (let i = 0; i < 8; i++) {
    const t = performance.now();
    scene.pickAt(scene.width / 2, scene.height / 2);
    pickSamples.push(performance.now() - t);
    if (performance.now() > pickDeadline) break;
  }

  // Label rebuild: the LOD pass in isolation.
  const labelSamples = [];
  if (labels) {
    for (let i = 0; i < 10; i++) {
      scene.labelsDirty = true;
      const t = performance.now();
      scene.updateLabels(performance.now(), true);
      labelSamples.push(performance.now() - t);
    }
  }

  const sortedFrames = frameSamples.slice().sort((a, b) => a - b);
  const sortedPicks = pickSamples.slice().sort((a, b) => a - b);
  const sortedLabels = labelSamples.slice().sort((a, b) => a - b);
  const info = scene.renderer.info;

  return {
    nodes: graph.nodeCount,
    edges: graph.edgeCount,
    labels,
    labelBudget,
    thickEdges,
    generateMs: round(generateMs),
    buildMs: round(buildMs),
    thickMs: round(thickMs),
    frameP50: round(percentile(sortedFrames, 0.5)),
    wallPerFrame: round(wallPerFrame),
    frameP95: round(percentile(sortedFrames, 0.95)),
    frameMax: round(sortedFrames[sortedFrames.length - 1] ?? 0),
    fps: round(1000 / Math.max(percentile(sortedFrames, 0.5), 0.001)),
    pickP50: round(percentile(sortedPicks, 0.5)),
    pickMax: round(sortedPicks[sortedPicks.length - 1] ?? 0),
    labelP50: round(percentile(sortedLabels, 0.5)),
    labelGlyphs: scene.labels ? scene.labels.glyphCount : 0,
    labelsDrawn: scene.labelCount ?? 0,
    labelsUnavailable: scene.labelsUnavailable === true,
    aborted,
    framesMeasured: frameSamples.length,
    drawCalls: info.render.calls,
    triangles: info.render.triangles,
    geometries: info.memory.geometries,
    heapMB: performance.memory
      ? round(performance.memory.usedJSHeapSize / (1024 * 1024))
      : null,
  };
}

function round(v) {
  return Math.round(v * 100) / 100;
}

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

export function currentScene() {
  return scene;
}

if (typeof window !== 'undefined') {
  window.__graphBench = { runCase, currentScene };
}
