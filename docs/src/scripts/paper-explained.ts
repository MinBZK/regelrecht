/*
 * Behavior for /research/rules-as-executed/uitgelegd and /explained.
 *
 * Progressive enhancement, as on the landing page. The server renders every
 * alternative of every switch, each under a visible label, and this file turns
 * them into one segmented control showing one alternative at a time. The
 * receipt panel only exists with JavaScript, because its decisions are computed
 * here, in the visitor's browser.
 */
import { runReceipts, recompute, type Receipts } from './paper-explained-run';

/** One segmented control, several panels; shows the panel the control selects. */
class Switch extends HTMLElement {
  private wired = false;

  connectedCallback() {
    if (this.wired) return;
    this.wired = true;
    const control = this.querySelector<HTMLElement & { value?: string }>('[data-control]');
    const panels = Array.from(this.querySelectorAll<HTMLElement>('[data-panel]'));
    if (!control || panels.length === 0) return;

    // The labels above each panel are for the stacked no-JS view; the control
    // names the visible panel once there is a control.
    this.querySelectorAll<HTMLElement>('.rr-px-panel__label').forEach((l) => (l.hidden = true));

    const show = (value: string) => {
      for (const p of panels) p.hidden = p.dataset.panel !== value;
    };
    const initial = this.dataset.default ?? panels[0].dataset.panel ?? '';
    control.setAttribute('value', initial);
    control.hidden = false;
    show(initial);

    control.addEventListener('change', (e) => {
      const value = (e as CustomEvent<{ value: string }>).detail?.value ?? control.value;
      if (value) show(value);
    });
  }
}

const money = (cents: number, lang: string) =>
  new Intl.NumberFormat(lang === 'en' ? 'en-GB' : 'nl-NL', { style: 'currency', currency: 'EUR' }).format(
    cents / 100,
  );

/** A ratio from the law file ("0.137") as the statute writes it: 13,7 %. */
const percent = (ratio: string, lang: string) =>
  new Intl.NumberFormat(lang === 'en' ? 'en-GB' : 'nl-NL', {
    style: 'percent',
    maximumFractionDigits: 2,
  }).format(Number(ratio));

const longDate = (iso: string, lang: string) =>
  new Intl.DateTimeFormat(lang === 'en' ? 'en-GB' : 'nl-NL', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  }).format(new Date(`${iso}T00:00:00`));

/** The version a law path names, by the date its file is named after. */
const versionOf = (path: string, lang: string, template: string) => {
  const from = path.split('/').pop()!.replace(/\.yaml$/, '');
  return template.replace('{date}', longDate(from, lang));
};

function formatFact(value: unknown, unit: string | null, lang: string, t: Strings): string {
  if (typeof value === 'number' && unit === 'eurocent') return money(value, lang);
  if (typeof value === 'number') return new Intl.NumberFormat(lang === 'en' ? 'en-GB' : 'nl-NL').format(value);
  if (typeof value === 'boolean') return value ? t.yes : t.no;
  if (value === null || value === undefined) return '–';
  return String(value);
}

interface Strings {
  ok: string;
  mismatch: string;
  checking: string;
  check: string;
  failed: string;
  revealA: string;
  inForce: string;
  inputs: string;
  yes: string;
  no: string;
}

class ReceiptsPanel extends HTMLElement {
  private observer?: IntersectionObserver;
  private receipts: Receipts | null = null;

  connectedCallback() {
    if (this.observer) return;
    const status = this.querySelector<HTMLElement>('[data-status]');
    if (status) status.hidden = false;
    // Fetch and compute once the panel is within a screen or so: a visitor
    // who never scrolls this far downloads neither the engine nor the laws.
    this.observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        this.observer?.disconnect();
        void this.compute(status);
      },
      { rootMargin: '1200px 0px' },
    );
    this.observer.observe(this);
  }

  disconnectedCallback() {
    this.observer?.disconnect();
  }

  private strings(): Strings {
    return JSON.parse(this.dataset.t ?? '{}');
  }

  private async compute(status: HTMLElement | null) {
    const lang = this.dataset.lang === 'en' ? 'en' : 'nl';
    const t = this.strings();
    try {
      const r = await runReceipts('/');
      this.receipts = r;
      const summary = t.inputs.replace('{n}', String(r.facts.length));
      for (const id of ['a', 'b'] as const) {
        const card = this.querySelector<HTMLElement>(`[data-receipt="${id}"]`);
        if (!card) continue;
        const d = r[id];
        const set = (f: string, v: string) => {
          const el = card.querySelector<HTMLElement>(`[data-f="${f}"]`);
          if (el) el.textContent = v;
        };
        set('version', versionOf(d.version, lang, t.inForce));
        set('digest', d.digest);
        set('date', longDate(r.calculationDate, lang));
        set('inputs', summary);
        set('amount', money(d.amount, lang));
        card.querySelector('[data-check-button]')?.addEventListener('click', () => void this.check(id, card));
      }

      const list = this.querySelector<HTMLElement>('[data-facts-list]');
      if (list) {
        list.replaceChildren(
          ...r.facts.map((f) => {
            const li = document.createElement('li');
            const code = document.createElement('code');
            code.textContent = `${f.provider}.${f.field}`;
            li.append(code, document.createTextNode(` ${formatFact(f.value, f.unit, lang, t)}`));
            return li;
          }),
        );
      }
      this.querySelector<HTMLElement>('[data-facts]')?.removeAttribute('hidden');
      this.querySelector<HTMLElement>('[data-receipts]')?.removeAttribute('hidden');
      if (status) status.hidden = true;
    } catch (err) {
      console.error('paper-explained:', err);
      if (status) status.textContent = t.failed;
    }
  }

  private async check(id: 'a' | 'b', card: HTMLElement) {
    const r = this.receipts;
    if (!r) return;
    const lang = this.dataset.lang === 'en' ? 'en' : 'nl';
    const t = this.strings();
    const button = card.querySelector<HTMLElement>('[data-check-button]');
    button?.setAttribute('loading', '');
    try {
      const verified = await recompute('/');
      const d = r[id];
      const digestOk = verified.digest === d.digest;
      const amountOk = verified.amount === d.amount;
      const tag = (name: string, ok: boolean) => {
        const el = card.querySelector<HTMLElement>(`[data-check="${name}"]`);
        if (!el) return;
        el.setAttribute('color', ok ? 'success' : 'critical');
        el.setAttribute('text', ok ? t.ok : t.mismatch);
      };
      tag('digest', digestOk);
      tag('amount', amountOk);
      card.querySelector<HTMLElement>('[data-checks]')?.removeAttribute('hidden');

      const reveal = card.querySelector<HTMLElement>('[data-reveal]');
      if (reveal) {
        reveal.textContent =
          id === 'a'
            ? t.revealA
            : (this.dataset.revealB ?? '')
                .replace('{published}', percent(r.change.published, lang))
                .replace('{local}', percent(r.change.local, lang))
                .replace('{diff}', money(Math.abs(d.amount - verified.amount), lang));
        reveal.hidden = false;
        // The button is about to go; keep the keyboard where the answer is.
        reveal.focus();
      }
      button?.setAttribute('hidden', '');
    } catch (err) {
      console.error('paper-explained:', err);
      const reveal = card.querySelector<HTMLElement>('[data-reveal]');
      if (reveal) {
        reveal.textContent = t.failed;
        reveal.hidden = false;
      }
    } finally {
      button?.removeAttribute('loading');
    }
  }
}

if (!customElements.get('rr-px-switch')) customElements.define('rr-px-switch', Switch);
if (!customElements.get('rr-px-receipts')) customElements.define('rr-px-receipts', ReceiptsPanel);
