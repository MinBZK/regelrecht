/**
 * Picking through a GPU id pass.
 *
 * A CPU raycast against an InstancedMesh is a linear scan over every instance
 * with a matrix inverse per hit test, so at 100.000 laws a hover costs more
 * than the frame it interrupts. Rendering the id of each instance as a colour
 * into a 1x1 render target costs one extra pass over the pixel under the
 * cursor and does not scale with the node count at all.
 *
 * The trick that makes it 1x1 rather than a full screen readback is
 * `camera.setViewOffset`: it maps a single pixel of the full viewport onto the
 * whole target, so the GPU still has to transform every vertex but rasterises
 * almost nothing.
 */

import { Color, WebGLRenderTarget, NearestFilter } from 'three';
import { decodePickId } from './nodeLayer.js';

export class GpuPicker {
  constructor(renderer, scene, camera, nodeLayer) {
    this.renderer = renderer;
    this.scene = scene;
    this.camera = camera;
    this.nodeLayer = nodeLayer;
    // A patch instead of a single pixel, and deliberately at 1:1 scale.
    //
    // `setViewOffset` onto a 1x1 target magnifies the viewport by its full
    // width: every triangle in the scene becomes hundreds of times larger in
    // clip space, and a rasteriser that bounds work by triangle size (any
    // tile-based one, SwiftShader included) then spends seconds on a pass that
    // writes one pixel. Measured on the real corpus: 68 seconds per pick at
    // 1x1 against 30 milliseconds for the patch below. Mapping a PATCH x PATCH
    // pixel region onto a PATCH x PATCH target keeps the scale at 1:1 and the
    // work proportional to what is actually drawn.
    this.patch = 32;
    this.target = new WebGLRenderTarget(this.patch, this.patch, {
      minFilter: NearestFilter,
      magFilter: NearestFilter,
      depthBuffer: true,
    });
    this.pixel = new Uint8Array(4);
    this.clearColor = new Color(0x000000);
    this.prevClear = new Color();
  }

  /**
   * @param {number} x CSS pixel x inside the canvas
   * @param {number} y CSS pixel y inside the canvas
   * @param {number} width canvas CSS width
   * @param {number} height canvas CSS height
   * @returns {number} node index, or -1 for background
   */
  pick(x, y, width, height) {
    const dpr = this.renderer.getPixelRatio();
    // setViewOffset is top-origin, exactly like the pointer coordinates, so no
    // flip happens anywhere: flipping once left the topmost pixel row unable to
    // hit anything at all.
    const px = Math.floor(x * dpr);
    const py = Math.floor(y * dpr);
    const w = Math.floor(width * dpr);
    const h = Math.floor(height * dpr);
    if (px < 0 || py < 0 || px >= w || py >= h) return -1;

    const camera = this.camera;
    const patch = this.patch;
    const half = patch >> 1;
    // Clamp so the patch always lies inside the viewport; the cursor then sits
    // off-centre near an edge, which the read-back offset below accounts for.
    const ox = Math.min(Math.max(px - half, 0), Math.max(0, w - patch));
    const oy = Math.min(Math.max(py - half, 0), Math.max(0, h - patch));
    camera.setViewOffset(w, h, ox, oy, patch, patch);

    const prevTarget = this.renderer.getRenderTarget();
    const prevClear = this.renderer.getClearColor(this.prevClear);
    const prevAlpha = this.renderer.getClearAlpha();

    this.nodeLayer.useMaterial('pick');
    this.hideNonPickable(true);
    this.renderer.setRenderTarget(this.target);
    this.renderer.setClearColor(this.clearColor, 1);
    this.renderer.clear();
    this.renderer.render(this.scene, camera);
    // Read the one pixel the cursor is on. The target's y axis runs bottom-up.
    const rx = Math.min(patch - 1, Math.max(0, px - ox));
    const ry = Math.min(patch - 1, Math.max(0, patch - 1 - (py - oy)));
    this.renderer.readRenderTargetPixels(this.target, rx, ry, 1, 1, this.pixel);
    this.renderer.setRenderTarget(prevTarget);
    this.renderer.setClearColor(prevClear, prevAlpha);
    this.hideNonPickable(false);
    this.nodeLayer.useMaterial('draw');
    camera.clearViewOffset();

    return decodePickId(this.pixel[0], this.pixel[1], this.pixel[2]);
  }

  /** Edges and labels must not write id colours; hide them for the pass. */
  hideNonPickable(hidden) {
    for (const obj of this.scene.children) {
      if (obj.userData.pickable !== false) continue;
      if (hidden) {
        obj.userData.wasVisible = obj.visible;
        obj.visible = false;
      } else {
        obj.visible = obj.userData.wasVisible ?? true;
      }
    }
  }

  dispose() {
    this.target.dispose();
  }
}
