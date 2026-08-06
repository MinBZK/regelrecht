/**
 * Rolling frame-time statistics.
 *
 * Average frame time is the wrong number for a renderer: a view that drops one
 * frame in ten still averages fine and still feels broken. What matters is the
 * tail, so this keeps a ring buffer and reports p50, p95 and the worst frame
 * in the window. Pure arithmetic, no timers - the caller feeds it timestamps.
 */

export class FrameStats {
  constructor(window = 120) {
    this.window = window;
    this.samples = new Float64Array(window);
    this.count = 0;
    this.cursor = 0;
    this.last = null;
  }

  /** Feed a timestamp (ms). The first call only sets the reference. */
  mark(now) {
    if (this.last !== null) this.push(now - this.last);
    this.last = now;
  }

  push(dt) {
    this.samples[this.cursor] = dt;
    this.cursor = (this.cursor + 1) % this.window;
    if (this.count < this.window) this.count++;
  }

  reset() {
    this.count = 0;
    this.cursor = 0;
    this.last = null;
  }

  summary() {
    if (this.count === 0) return { count: 0, p50: 0, p95: 0, max: 0, fps: 0 };
    const arr = Array.from(this.samples.subarray(0, this.count)).sort((a, b) => a - b);
    const at = (q) => arr[Math.min(arr.length - 1, Math.floor(q * (arr.length - 1)))];
    const p50 = at(0.5);
    return {
      count: arr.length,
      p50,
      p95: at(0.95),
      max: arr[arr.length - 1],
      fps: p50 > 0 ? 1000 / p50 : 0,
    };
  }
}
