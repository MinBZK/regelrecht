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
  private liveRunning = false;
  /** The engine module, once fetching has started. */
  private enginePromise: Promise<typeof import('~/scripts/landing-run.ts')> | null = null;
  /** Suppresses the replay button until there is a run to replay. */
  private replaySuppressed = false;

  /**
   * Hide or show the replay button, remembering the choice.
   *
   * `startRun` unhides it whenever a run finishes, so the suppression has to
   * live somewhere that survives that; a flag here is read back on every such
   * reveal rather than fought over with attributes.
   */
  private setReplayHidden(hidden: boolean) {
    this.replaySuppressed = hidden;
    const replay = this.querySelector<HTMLElement>('[data-replay]');
    if (replay) replay.hidden = hidden;
  }

  connectedCallback() {
    const lang = this.dataset.lang === 'en' ? 'en' : 'nl';

    this.setAttribute('data-enhanced', 'true');
    // Every wipe on the page, however many there are: the panels differ in
    // what they show, not in how they behave.
    this.querySelectorAll<HTMLElement>('[data-wipe]').forEach((wipe) => {
      const viewer = wipe.querySelector('.rr-wipe__pane--under nldd-code-viewer');
      if (viewer) void this.capCodeViewer(viewer);
      this.initWipe(wipe);
    });
    this.initValues(lang);
    this.initReveal(lang);
  }

  disconnectedCallback() {
    this.observer?.disconnect();
  }

  /** Format the values of the rows the run produced. */
  private initValues(lang: string) {
    this.querySelectorAll<HTMLElement>('[data-value]').forEach((el) => {
      el.textContent = formatValue(
        el.dataset.value ?? 'null',
        el.dataset.unit ?? '',
        lang,
      );
    });

    // The total stays empty until the last step lands: an outcome that is
    // already on screen while the steps are still arriving gives the
    // walk-through nothing to arrive at.
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
  private async capCodeViewer(viewer: Element) {
    if (typeof CSSStyleSheet === 'undefined') return;

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

      // Padding goes in the same sheet: the code sat flush against the frame,
      // and the viewer's own `simple` variant carries none. Room at the top is
      // left for the copy button so the first line does not run under it.
      // The scrollbars are hidden, the scrolling is not: the data tables in a
      // scenario are wider than the frame and still have to be reachable, but
      // a bar drawn across the bottom of a code block reads as a defect.
      sheet.replaceSync(
        `.code-viewer, .cm-editor { height: ${height}px; max-height: ${height}px; }` +
          ' .cm-scroller { overflow: auto; padding: 1rem 1.25rem 1.25rem;' +
          ' scrollbar-width: none; -ms-overflow-style: none; }' +
          ' .cm-scroller::-webkit-scrollbar { width: 0; height: 0; }' +
          ' .cm-content { padding-inline-end: 3rem; }',
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
  private initWipe(wipe: HTMLElement) {
    const input = wipe.querySelector<HTMLInputElement>('[data-wipe-input]');
    if (!input) return;

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

    // Where the page stood when this panel first reached the bottom edge.
    let armedAt: number | null = null;

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

        // The wipe holds until the panel is fully in view, and only then
        // follows the scroll. Reading comes first: the covering text is there
        // to be read, and a reveal that starts while the panel is still coming
        // up takes it away before it has been.
        //
        // So the travel runs from "the panel's bottom has reached the bottom of
        // the viewport", which is the moment it is all on screen, to a panel
        // height further on. Measured against the panel rather than against
        // fixed fractions of the window, because a tall panel needs longer to
        // arrive than a short one.
        const start = viewport - box.height;
        const end = start - box.height;
        const travelled = (start - box.top) / (start - end);

        // Position alone is not enough. On a short page, or when the panel
        // already sits high at rest, it would start part-way open before the
        // visitor has scrolled at all: the statute must be readable in full
        // first, or the promise of a before-and-after is broken on arrival.
        // So the panel's own travel is capped by how far the page has scrolled
        // since this panel first came into view, measured over half a viewport.
        // Measured per panel, not from the top of the document: the second wipe
        // sits far down the page, where `scrollY` is large before it is even in
        // sight, and the cap would never bite.
        if (armedAt === null && box.top <= viewport) armedAt = window.scrollY;
        const scrolled =
          armedAt === null ? 0 : (window.scrollY - armedAt) / (viewport * 0.5);
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

    // Wired before the branches below, because the reduced-motion path returns
    // early: it used to skip this and leave a visible "run again" button that
    // did nothing at all, for the visitors least able to shrug that off.
    const replay = this.querySelector<HTMLButtonElement>('[data-replay]');
    replay?.addEventListener('click', () => {
      this.hasRun = false;
      void this.liveRun(lang);
    });
    // The button appears only once there is a live trace to replay.
    this.setReplayHidden(true);

    if (reduceMotion() || !('IntersectionObserver' in window)) {
      beats.forEach((b) => b.setAttribute('data-visible', 'true'));
      // Still run: the trace is content, not decoration. `startRun` checks the
      // preference again and puts the rows up in one go.
      void this.liveRun(lang);
      return;
    }

    this.observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (!entry.isIntersecting) return;
          const el = entry.target as HTMLElement;
          this.observerFired = true;
          el.setAttribute('data-visible', 'true');
          // The run panel starts the engine when it comes into view: the
          // animation is the point of arriving there, and it should not wait
          // for a press. Fetching begins earlier, when the panel before it
          // appears, so the download is usually done by the time this fires.
          if (el.querySelector('[data-run]')) {
            void this.liveRun(lang);
          } else {
            void this.warmEngine();
          }
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
      // A panel that is already on screen when the page loads -- someone
      // following a link straight to #demo, or a short page on a tall window --
      // gets the same treatment as one scrolled into view. The observer reports
      // transitions, so without this the run would simply never start there.
      if (onScreen) {
        if (b.querySelector('[data-run]')) void this.liveRun(lang);
        else void this.warmEngine();
      }
    });

    // A last resort: if the observer has not fired at all a few seconds in,
    // show everything. Better a section that appears without ceremony than one
    // the visitor never sees.
    //
    // The condition is whether the observer ever ran, not whether anything is
    // visible: a first panel revealed here because it was already on screen
    // would otherwise satisfy the check while every panel below it stayed
    // hidden forever.
    // It reveals the panels and starts the run too. An earlier version left the
    // run alone, on the reasoning that it must not be over before the visitor
    // arrives; that held while the panel shipped with a recording to fall back
    // on. It no longer does, so a run that never starts is simply an empty
    // panel.
    window.setTimeout(() => {
      if (this.observerFired) return;
      beats.forEach((b) => b.setAttribute('data-visible', 'true'));
      void this.liveRun(lang);
    }, 4000);

  }

  /**
   * Start fetching the engine and the laws, without running anything yet.
   *
   * Called when an earlier panel scrolls into view, so the roughly 600 KB is
   * usually on the machine by the time the run panel arrives and the animation
   * can begin at once. Failure is silent here: the run itself reports it.
   */
  private async warmEngine(): Promise<void> {
    if (this.enginePromise) return;
    this.enginePromise = import('~/scripts/landing-run.ts')
      .then((m) => m.prepare(import.meta.env.BASE_URL ?? '/').then(() => m))
      .catch((err) => {
        this.enginePromise = null;
        throw err;
      });
    try {
      await this.enginePromise;
    } catch {
      // Reported when the run is attempted, not while warming up.
    }
  }

  /**
   * Run the scenario here and let the player walk through what comes back.
   *
   * No button: reaching this panel is the request. The engine is what the panel
   * is about, so waiting for a press would mean an empty frame at exactly the
   * moment the visitor arrived to see it fill.
   */
  private async liveRun(lang: string): Promise<void> {
    if (this.liveRunning || this.hasRun) return;
    this.liveRunning = true;

    const label = (key: string, fallback: string) => this.dataset[key] ?? fallback;
    const empty = this.querySelector<HTMLElement>('[data-empty]');
    const status = this.querySelector<HTMLElement>('[data-run-status]');
    if (empty) empty.textContent = label('tLoading', 'Loading…');
    if (status) status.textContent = label('tLoading', 'Loading…');

    try {
      await this.warmEngine();
      const mod = await (this.enginePromise ?? import('~/scripts/landing-run.ts'));
      const result = await mod.runScenario(import.meta.env.BASE_URL ?? '/');
      this.renderLive(result, lang);
    } catch (err) {
      console.error('[landing] live run failed', err);
      if (empty) {
        empty.hidden = false;
        empty.textContent = label('tFailed', 'The run failed.');
      }
      if (status) status.textContent = '';
    } finally {
      this.liveRunning = false;
    }
  }

  /**
   * Swap the recorded rows for the ones that just ran.
   *
   * Only the rows change. The tree, the columns and the replay are the same
   * code that renders the recording, so what a visitor sees after pressing the
   * button is the same panel showing a different run -- not a second design.
   */
  private renderLive(result: import('~/scripts/landing-run.ts').RunResult, lang: string) {
    const list = this.querySelector<HTMLElement>('.rr-trace');
    if (!list) return;

    // Readable names, passed in as data by the component: the kinds of step,
    // and the law names, in the page's language.
    let labels: { outputs: Record<string, string>; laws: Record<string, string>; kinds: Record<string, string> };
    try {
      labels = JSON.parse(this.dataset.labels ?? '{}');
    } catch {
      labels = { outputs: {}, laws: {}, kinds: {} };
    }
    const kindLabels = labels.kinds ?? {};
    const lawLabels = labels.laws ?? {};

    // Nothing to explain once there are rows.
    const empty = this.querySelector<HTMLElement>('[data-empty]');
    if (empty) empty.hidden = true;

    list.textContent = '';
    result.beats.forEach((b, i) => {
      const row = document.createElement('li');
      row.className = 'rr-trace__row';
      row.dataset.step = String(i);
      row.dataset.type = b.type;

      const kind = document.createElement('span');
      kind.className = 'rr-trace__kind';
      kind.dataset.kind = b.type;
      kind.textContent = kindLabels[b.type] ?? b.type;

      const name = document.createElement('span');
      name.className = 'rr-trace__name';
      const tree = document.createElement('span');
      tree.className = 'rr-trace__tree';
      tree.setAttribute('aria-hidden', 'true');
      tree.textContent = b.tree;
      const code = document.createElement('code');
      code.textContent = b.identifier;
      name.append(tree, code);
      if (b.law) {
        const law = document.createElement('span');
        law.className = 'rr-trace__law';
        law.textContent = lawLabels[b.law] ?? b.law.replace(/_/g, ' ');
        name.append(law);
      }

      const value = document.createElement('span');
      value.className = 'rr-trace__value';
      value.dataset.value = JSON.stringify(b.result ?? null);
      value.dataset.unit = b.unit ?? '';

      // No per-step timing column. The engine cannot measure one under
      // WebAssembly -- `Instant::now()` trips RefCell aliasing in
      // wasm-bindgen, so the traced entry points force an untimed builder and
      // `duration_us` is always absent (RFC-039 says no consumer may depend on
      // it). The recording used to carry them because it ran natively. An
      // empty column on 136 rows would read as "took no time" rather than "not
      // measured here", so the column is gone; the total is measured in the
      // page and stated under the trace.
      row.append(kind, name, value);
      list.append(row);
    });

    // The amount this run produced, which the replay reveals at the last step.
    this.dataset.amount = String(result.amount ?? 'null');

    const facts = this.querySelector<HTMLElement>('[data-facts]');
    if (facts) {
      const ms = result.durationMs.toFixed(2).replace('.', lang === 'en' ? '.' : ',');
      const pre = this.dataset.tLive ?? '';
      // Say that the walk-through is slowed down. The engine takes a couple of
      // milliseconds and the animation takes seconds, so a visitor who watches
      // 136 steps crawl past and then reads "1.60 ms" has every reason to
      // distrust the number. The measurement is real; the pace is a choice.
      const slow = this.dataset.tSlow ?? '';
      facts.textContent =
        lang === 'en'
          ? `${pre} ${result.steps} steps in ${ms} ms. ${slow}`
          : `${pre} ${result.steps} stappen in ${ms} ms. ${slow}`;
    }

    this.initValues(lang);
    // There is a live trace now, so replaying it means something: it walks
    // through the run that just happened, not a stand-in for one.
    this.setReplayHidden(false);
    this.hasRun = false;
    this.startRun(lang);
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

    // Reduced motion: the trace is content, the walk-through is decoration, so
    // put every row up at once and be done. Checked here rather than at the
    // call sites, because every path into the player wants the same answer.
    if (reduceMotion()) {
      steps.forEach((s) => s.setAttribute('data-done', 'true'));
      run.setAttribute('data-state', 'done');
      run.setAttribute('data-complete', 'true');
      if (status) status.textContent = labels.done;
      if (replay) replay.hidden = this.replaySuppressed;
      this.showAmount(lang);
      return;
    }

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
          if (replay) replay.hidden = this.replaySuppressed;
          this.showAmount(lang);
        }
      }, i * interval);
    });
  }

}

if (!customElements.get('rr-scrolly')) {
  customElements.define('rr-scrolly', ScrollyDemo);
}
