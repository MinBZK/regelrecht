/*
 * Behaviour for the scrollytelling demo.
 *
 * Progressive enhancement throughout: without this script every panel is
 * visible and readable, the wipe shows the statute, and the run panel lists its
 * steps with their recorded results. This file only adds the reveal, the wipe
 * and the replay.
 *
 * Nothing here computes anything. The values come from the recorded trace, and
 * the replay decides only *when* to show a step that was already taken.
 */

/**
 * How long the replay takes from first step to last, in milliseconds.
 *
 * A fixed pace per step would run for half a minute over a full trace, so the
 * whole walk-through is given a budget and the interval follows from the number
 * of steps. Long enough to read as a sequence, short enough to sit through.
 */
const RUN_MS = 6000;

/** Never faster than this, so a short trace still reads as one step at a time. */
const MIN_STEP_MS = 40;

const reduceMotion = () =>
  window.matchMedia('(prefers-reduced-motion: reduce)').matches;

/**
 * Format a recorded result for display.
 *
 * Amounts are stored in eurocent, the unit the law declares, so a value is only
 * divided when its step says it is money. Anything else is shown as the engine
 * produced it.
 */
function formatValue(raw: string, unit: string, lang: string): string {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return '';
  }
  if (value === null || value === undefined) return '';
  if (typeof value === 'boolean') {
    if (lang === 'en') return value ? 'yes' : 'no';
    return value ? 'ja' : 'nee';
  }
  if (typeof value === 'number') {
    if (unit === 'eurocent') {
      return new Intl.NumberFormat(lang === 'en' ? 'en-GB' : 'nl-NL', {
        style: 'currency',
        currency: 'EUR',
      }).format(value / 100);
    }
    return new Intl.NumberFormat(lang === 'en' ? 'en-GB' : 'nl-NL').format(value);
  }
  if (typeof value === 'string') return value;
  return '';
}

class ScrollyDemo extends HTMLElement {
  private observer?: IntersectionObserver;
  private hasRun = false;
  private observerFired = false;

  connectedCallback() {
    const lang = this.dataset.lang === 'en' ? 'en' : 'nl';

    this.setAttribute('data-enhanced', 'true');
    this.capCodeViewer();
    this.initWipe();
    this.initValues(lang);
    this.initReveal(lang);
  }

  disconnectedCallback() {
    this.observer?.disconnect();
  }

  /** The recorded results, rendered once so they are right without JS timing. */
  private initValues(lang: string) {
    this.querySelectorAll<HTMLElement>('[data-value]').forEach((el) => {
      el.textContent = formatValue(
        el.dataset.value ?? 'null',
        el.dataset.unit ?? '',
        lang,
      );
    });

    // The total is left empty here on purpose. Without JS the server-rendered
    // amount stands in the markup and is simply read; with JS the replay fills
    // it in at the end, because an outcome that is already on screen while the
    // steps are still arriving gives the walk-through nothing to arrive at.
    const out = this.querySelector<HTMLOutputElement>('[data-amount-out]');
    if (out) out.textContent = '';
  }

  /** Reveal the computed total, in the same formatting the steps use. */
  private showAmount(lang: string) {
    const out = this.querySelector<HTMLOutputElement>('[data-amount-out]');
    if (out) {
      out.textContent = formatValue(this.dataset.amount ?? 'null', 'eurocent', lang);
    }
  }

  /**
   * Let the YAML scroll inside its own frame.
   *
   * nldd-code-viewer sizes itself to its content and offers no way in from
   * outside: no `part`, no height custom property, and a height on the host
   * leaves the element inside its shadow root free to grow, so its scroller
   * never learns it is out of room. Without this the panel stretched to some
   * 2700 pixels. A stylesheet adopted into the shadow root caps the element the
   * scroller measures against, after which the viewer's own scrolling works as
   * built. Reported to the user: capping it from the outside is what the
   * component is missing.
   */
  private async capCodeViewer() {
    const viewer = this.querySelector('.rr-wipe__pane--under nldd-code-viewer');
    if (!viewer || typeof CSSStyleSheet === 'undefined') return;

    // The shadow root does not exist yet when this element connects, so wait
    // for the component to be defined and to finish its first render.
    try {
      await customElements.whenDefined('nldd-code-viewer');
      await (viewer as any).updateComplete;
    } catch {
      // Keep going: the root may be there anyway.
    }

    const root = (viewer as any).shadowRoot as ShadowRoot | undefined;
    if (!root) return;

    try {
      const sheet = new CSSStyleSheet();
      // A pixel height, not a percentage: `100%` resolves against a parent
      // that has none of its own here, so the editor kept growing to fit its
      // content and its scroller never saw a limit. The pane's own height is
      // the figure to hold it to.
      const pane = viewer.closest('.rr-wipe__pane--under') as HTMLElement | null;
      const height = Math.round(pane?.getBoundingClientRect().height ?? 0);
      if (height <= 0) return;

      sheet.replaceSync(
        `.code-viewer, .cm-editor { height: ${height}px; max-height: ${height}px; }` +
          ' .cm-scroller { overflow: auto; }',
      );
      root.adoptedStyleSheets = [...root.adoptedStyleSheets, sheet];
    } catch {
      // Constructable stylesheets are not available everywhere; without them
      // the YAML simply shows down to the fold, which is what it did before.
    }
  }

  /**
   * The wipe between the statute and the machine-readable rule.
   *
   * Driven by a real range input, so it works with a pointer, with a keyboard
   * and with assistive technology. Scrolling nudges the same input, which is
   * what makes it feel scroll-driven without taking the control away.
   */
  private initWipe() {
    const wipe = this.querySelector<HTMLElement>('[data-wipe]');
    const input = this.querySelector<HTMLInputElement>('[data-wipe-input]');
    if (!wipe || !input) return;

    const apply = (pct: number) => {
      wipe.style.setProperty('--rr-wipe', `${pct}%`);
      input.setAttribute('aria-valuetext', `${Math.round(pct)}%`);
    };

    apply(Number(input.value));
    input.addEventListener('input', () => apply(Number(input.value)));

    if (reduceMotion()) return;

    // Scroll position maps to the wipe only while the panel is on screen, and
    // only until the visitor touches the control: after that the position is
    // theirs, not the page's.
    let userOwned = false;
    input.addEventListener('pointerdown', () => (userOwned = true));
    input.addEventListener('keydown', () => (userOwned = true));

    // Coalesced into one frame: a scroll listener that measures and writes on
    // every event forces layout per event and can wedge the page.
    let queued = false;
    const onScroll = () => {
      if (userOwned || queued) return;
      queued = true;
      window.requestAnimationFrame(() => {
        queued = false;
        if (userOwned) return;
        const box = wipe.getBoundingClientRect();
        const viewport = window.innerHeight;

        // The wipe follows the panel's travel up the viewport, from "its top
        // has reached three quarters up" to "its top is halfway up". Ending
        // halfway rather than near the top means the YAML is fully uncovered
        // while the panel is still square in view: run it to the top and the
        // reveal only completes as the panel is leaving.
        const start = viewport * 0.75;
        const end = viewport * 0.5;
        const travelled = (start - box.top) / (start - end);

        // Position alone is not enough. On a short page, or when the panel
        // already sits high at rest, it would start part-way open before the
        // visitor has scrolled at all: the statute must be readable in full
        // first, or the promise of a before-and-after is broken on arrival.
        // So the panel's own travel is capped by how far the page has actually
        // been scrolled, measured over half a viewport: enough to hold the wipe
        // shut on arrival without slowing the rest of it down.
        const scrolled = window.scrollY / (viewport * 0.5);
        const progress = Math.min(1, Math.max(0, Math.min(travelled, scrolled)));
        // 100% is the statute covering the whole pane, 0% is the YAML fully
        // uncovered. Scrolling down reveals the machine-readable rule, so
        // progress counts down: the further you scroll, the more is shown.
        const pct = Math.round((1 - progress) * 100);
        if (String(pct) !== input.value) input.value = String(pct);
        apply(pct);
      });
    };

    window.addEventListener('scroll', onScroll, { passive: true });
    onScroll();
  }

  /** Reveal each beat as it comes into view, and start the run at the last one. */
  private initReveal(lang: string) {
    const beats = Array.from(this.querySelectorAll<HTMLElement>('[data-beat]'));
    if (beats.length === 0) return;

    if (reduceMotion() || !('IntersectionObserver' in window)) {
      beats.forEach((b) => b.setAttribute('data-visible', 'true'));
      this.finishRun(lang);
      return;
    }

    this.observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (!entry.isIntersecting) return;
          const el = entry.target as HTMLElement;
          this.observerFired = true;
          el.setAttribute('data-visible', 'true');
          if (el.querySelector('[data-run]')) this.startRun(lang);
        });
      },
      // A panel can be taller than the viewport (the scenario one is), and a
      // fractional threshold is a fraction of the *element*, so 0.25 of a
      // 1300px panel can never be met in a 950px window: the observer would
      // simply never fire for it. Threshold 0 asks the only question that
      // always has an answer, "is any of it showing", and the bottom margin
      // holds the reveal back until the panel is properly in view.
      { rootMargin: '0px 0px -20% 0px', threshold: 0 },
    );

    // Hide only once the observer is actually watching, and only what is not
    // already on screen. Hiding first would leave the whole section invisible
    // if the observer never fired, which is a worse failure than no animation:
    // the content is the point, the reveal is decoration.
    beats.forEach((b) => {
      const box = b.getBoundingClientRect();
      const onScreen = box.top < window.innerHeight && box.bottom > 0;
      b.setAttribute('data-visible', onScreen ? 'true' : 'false');
      this.observer?.observe(b);
    });

    // A last resort: if the observer has not fired at all a few seconds in,
    // show everything. Better a section that appears without ceremony than one
    // the visitor never sees.
    //
    // The condition is whether the observer ever ran, not whether anything is
    // visible: a first panel revealed here because it was already on screen
    // would otherwise satisfy the check while every panel below it stayed
    // hidden forever.
    // It reveals the panels but does not finish the run: the run is the one
    // thing that must not be over before the visitor has reached it, and a
    // fallback that completes it would put the outcome on screen while the
    // steps are still waiting to be walked.
    window.setTimeout(() => {
      if (this.observerFired) return;
      beats.forEach((b) => b.setAttribute('data-visible', 'true'));
    }, 4000);

    const replay = this.querySelector<HTMLButtonElement>('[data-replay]');
    replay?.addEventListener('click', () => {
      this.hasRun = false;
      this.startRun(lang);
    });
  }

  /**
   * Replay the recorded steps at a readable pace.
   *
   * The engine took about three milliseconds; this is the display choosing to
   * be slower, not the engine being slow. The panel says what the real time
   * was, so the animation never stands in for the measurement.
   */
  private startRun(lang: string) {
    if (this.hasRun) return;
    this.hasRun = true;

    const run = this.querySelector<HTMLElement>('[data-run]');
    const status = this.querySelector<HTMLElement>('[data-run-status]');
    const replay = this.querySelector<HTMLButtonElement>('[data-replay]');
    const steps = Array.from(this.querySelectorAll<HTMLElement>('[data-step]'));
    if (!run) return;

    const labels =
      lang === 'en'
        ? { running: 'Running', done: 'Done' }
        : { running: 'Bezig met uitvoeren', done: 'Klaar' };

    run.setAttribute('data-state', 'running');
    if (status) status.textContent = labels.running;
    if (replay) replay.hidden = true;
    steps.forEach((s) => s.setAttribute('data-done', 'false'));
    run.removeAttribute('data-complete');

    const scroller = this.querySelector<HTMLElement>('[data-steps-scroll]');
    if (scroller) scroller.scrollTop = 0;
    const interval = Math.max(MIN_STEP_MS, RUN_MS / Math.max(1, steps.length));

    steps.forEach((step, i) => {
      window.setTimeout(() => {
        step.setAttribute('data-done', 'true');

        // Follow the step that just landed, so a long trace plays out in view
        // instead of running on below the fold. Only while the replay owns the
        // list: once it is done the visitor scrolls it themselves.
        if (scroller && i < steps.length - 1) {
          const top = step.offsetTop - scroller.clientHeight * 0.6;
          if (top > scroller.scrollTop) scroller.scrollTop = top;
        }

        if (i === steps.length - 1) {
          run.setAttribute('data-state', 'done');
          run.setAttribute('data-complete', 'true');
          if (status) status.textContent = labels.done;
          if (replay) replay.hidden = false;
          this.showAmount(lang);
        }
      }, i * interval);
    });
  }

  /** Show the finished state without animating, for reduced motion. */
  private finishRun(lang: string) {
    const run = this.querySelector<HTMLElement>('[data-run]');
    const status = this.querySelector<HTMLElement>('[data-run-status]');
    if (!run) return;
    this.hasRun = true;
    run.setAttribute('data-state', 'done');
    run.setAttribute('data-complete', 'true');
    this.querySelectorAll<HTMLElement>('[data-step]').forEach((s) =>
      s.setAttribute('data-done', 'true'),
    );
    if (status) status.textContent = lang === 'en' ? 'Done' : 'Klaar';
    this.showAmount(lang);
  }
}

if (!customElements.get('rr-scrolly')) {
  customElements.define('rr-scrolly', ScrollyDemo);
}
