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

/** Steps appear this far apart while replaying. */
const STEP_MS = 260;

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

    const out = this.querySelector<HTMLOutputElement>('[data-amount-out]');
    if (out) {
      out.textContent = formatValue(this.dataset.amount ?? 'null', 'eurocent', lang);
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

        // The wipe stays shut until the panel has properly arrived, and only
        // then follows the scroll. Measuring from the moment the panel first
        // touches the bottom edge would have it half open before the visitor
        // has scrolled at all, which is what it used to do.
        //
        // It runs from "the panel's top has reached three quarters up the
        // viewport" to "the panel's top has reached one fifth up", a stretch
        // the visitor crosses while the panel is the thing they are looking at.
        const start = viewport * 0.75;
        const end = viewport * 0.2;
        const travelled = (start - box.top) / (start - end);
        const progress = Math.min(1, Math.max(0, travelled));
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
      { rootMargin: '0px 0px -15% 0px', threshold: 0.25 },
    );

    // Hide only once the observer is actually watching, and only what is not
    // already on screen. Hiding first would leave the whole section invisible
    // if the observer never fired, which is a worse failure than no animation:
    // the content is the point, the reveal is decoration.
    beats.forEach((b) => {
      const box = b.getBoundingClientRect();
      const onScreen = box.top < window.innerHeight && box.bottom > 0;
      b.setAttribute('data-visible', onScreen ? 'true' : 'false');
      if (onScreen && b.querySelector('[data-run]')) this.startRun(lang);
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
    window.setTimeout(() => {
      if (this.observerFired) return;
      beats.forEach((b) => b.setAttribute('data-visible', 'true'));
      this.finishRun(lang);
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

    steps.forEach((step, i) => {
      window.setTimeout(() => {
        step.setAttribute('data-done', 'true');
        if (i === steps.length - 1) {
          run.setAttribute('data-state', 'done');
          run.setAttribute('data-complete', 'true');
          if (status) status.textContent = labels.done;
          if (replay) replay.hidden = false;
        }
      }, i * STEP_MS);
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
  }
}

if (!customElements.get('rr-scrolly')) {
  customElements.define('rr-scrolly', ScrollyDemo);
}
