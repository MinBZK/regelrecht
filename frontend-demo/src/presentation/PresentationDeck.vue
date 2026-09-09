<script setup>
import { computed } from 'vue';
import { usePresentation } from './usePresentation.js';
import { useDemo } from '../store/demoStore.js';

// The deck: a Rijkshuisstijl-blue panel, full-screen for the intro and the
// closing, a left rail while the live demo runs on the right. Slides are data
// (demo-config.yaml); this component only knows how to render each kind.

const p = usePresentation();
const { state } = useDemo();

const today = new Date().toLocaleDateString('nl-NL', { day: 'numeric', month: 'long', year: 'numeric' });
const counter = computed(() => `${p.index.value + 1} / ${p.total.value}`);
const progress = computed(() => (p.total.value ? `${((p.index.value + 1) / p.total.value) * 100}%` : '0%'));
const isLast = computed(() => p.index.value === p.total.value - 1);

/** `**bold**` in a statement line → <strong>, everything else escaped. */
function emphasize(line) {
  const escaped = String(line).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
  return escaped.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
}
function saveName(e) {
  state.presenterName = e.target?.value ?? state.presenterName;
}
</script>

<template>
  <Teleport to="body">
    <div v-if="p.active.value && p.current.value" class="deck" :class="{ full: p.isFull.value }" role="region" aria-label="Presentatie">
      <div class="content">
        <div class="content-main">
          <!-- Title slide -->
          <template v-if="p.current.value.kind === 'title'">
            <span class="overline">{{ today }}</span>
            <h1 class="title title-hero">{{ p.current.value.title }}</h1>
            <p v-if="p.current.value.subtitle" class="lead lead-hero">{{ p.current.value.subtitle }}</p>
            <div class="title-meta">
              <input class="presenter" :value="state.presenterName" placeholder="Naam presentator" aria-label="Naam presentator" @change="saveName" />
              <span v-if="p.current.value.footer" class="affiliation">{{ p.current.value.footer }}</span>
            </div>
          </template>

          <!-- Statement slide: a few big lines -->
          <template v-else-if="p.current.value.kind === 'statement'">
            <span v-if="p.current.value.overline" class="overline">{{ p.current.value.overline }}</span>
            <h1 class="title statement">
              <template v-for="(line, j) in p.current.value.lines" :key="j">
                <span v-html="emphasize(line)"></span><br v-if="j < p.current.value.lines.length - 1" />
              </template>
            </h1>
          </template>

          <!-- Closing slide -->
          <template v-else-if="p.current.value.kind === 'closing'">
            <span v-if="p.current.value.overline" class="overline">{{ p.current.value.overline }}</span>
            <h1 class="title title-hero">{{ p.current.value.title }}</h1>
            <ul v-if="p.current.value.lines?.length" class="bullets bullets-plain">
              <li v-for="(line, j) in p.current.value.lines" :key="j" v-html="emphasize(line)"></li>
            </ul>
            <a v-if="p.current.value.link" class="slide-link" :href="p.current.value.link.href" target="_blank" rel="noopener noreferrer">{{ p.current.value.link.label }}</a>
          </template>

          <!-- Demo slide: the rail next to the live app -->
          <template v-else>
            <span v-if="p.current.value.overline" class="overline">{{ p.current.value.overline }}</span>
            <h1 class="title">{{ p.current.value.title }}</h1>
            <p v-if="p.current.value.lead" class="lead">{{ p.current.value.lead }}</p>
            <ul v-if="p.current.value.bullets?.length" class="bullets">
              <li v-for="(b, j) in p.current.value.bullets" :key="j" v-html="emphasize(b)"></li>
            </ul>
          </template>
        </div>
        <div v-if="p.current.value.note" class="content-foot">
          <p class="note">{{ p.current.value.note }}</p>
        </div>
      </div>

      <div class="footer">
        <div class="footer-row">
          <span class="counter" :aria-label="`Dia ${p.index.value + 1} van ${p.total.value}`">{{ counter }}</span>
          <div class="nav">
            <button type="button" class="round" aria-label="Vorige dia" :disabled="p.index.value === 0" @click="p.prev()">
              <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M15 5l-7 7 7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
            </button>
            <button v-if="isLast" type="button" class="pill" @click="p.stop()">Sluiten</button>
            <button v-else type="button" class="round" aria-label="Volgende dia" @click="p.next()">
              <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M9 5l7 7-7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
            </button>
          </div>
        </div>
        <div class="hints">
          <span>pijltjes of spatie</span>
          <span>Esc sluit</span>
          <span>f volledig scherm</span>
        </div>
      </div>
      <div class="progress" aria-hidden="true"><div class="progress-fill" :style="{ width: progress }"></div></div>
    </div>
  </Teleport>
</template>

<style scoped>
/* Custom CSS on purpose: the design system has no presentation component. The
 * palette is the Rijkshuisstijl (donkerblauw #154273, lintblauw #01689b), the
 * type is RijksoverheidSerif for titles and RijksSans for the rest. */
.deck {
  position: fixed;
  inset: 0 auto 0 0;
  width: 36vw;
  z-index: 80;
  display: flex;
  flex-direction: column;
  padding: 3rem 2.75rem 1.5rem;
  box-sizing: border-box;
  color: #fff;
  background: linear-gradient(160deg, #154273 0%, #1a4f86 100%);
  box-shadow: 4px 0 24px rgba(0, 0, 0, 0.25);
  font-family: 'RijksSans', system-ui, sans-serif;
  transition: width 0.5s cubic-bezier(0.22, 1, 0.36, 1), padding 0.5s cubic-bezier(0.22, 1, 0.36, 1);
}
.deck.full {
  width: 100vw;
  padding: 4rem clamp(3rem, 12vw, 14rem) 2rem;
}

.content {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: auto;
}
.content-main {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 1.4rem;
}
.content-foot {
  flex: 0 0 auto;
  padding-top: 1.25rem;
  margin-top: 1.5rem;
  border-top: 1px solid rgba(255, 255, 255, 0.18);
}

.overline {
  font-size: clamp(1rem, 1.5vw, 1.6rem);
  font-weight: 600;
  letter-spacing: 0.02em;
  color: rgba(255, 255, 255, 0.72);
}
.title {
  font-family: 'RijksoverheidSerif', Georgia, serif;
  font-weight: 700;
  font-size: clamp(2.4rem, 4.2vw, 5.4rem);
  line-height: 1.08;
  margin: 0;
  color: #fff;
}
.title-hero {
  font-size: clamp(3.2rem, 7vw, 9rem);
}
.statement {
  font-size: clamp(2.4rem, 4.8vw, 6.2rem);
  line-height: 1.15;
  font-weight: 400;
}
.statement :deep(strong) {
  font-weight: 700;
}
.lead {
  font-size: clamp(1.3rem, 2vw, 2.3rem);
  line-height: 1.4;
  margin: 0;
  font-weight: 500;
}
.lead-hero {
  font-family: 'RijksoverheidSerif', Georgia, serif;
  font-style: italic;
  font-weight: 400;
  font-size: clamp(1.6rem, 3vw, 3.6rem);
  color: rgba(255, 255, 255, 0.9);
}
.bullets {
  font-size: clamp(1.15rem, 1.75vw, 2rem);
  line-height: 1.45;
  margin: 0;
  padding-left: 1.3rem;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
  color: rgba(255, 255, 255, 0.96);
}
.bullets li::marker {
  color: rgba(255, 255, 255, 0.7);
}
.bullets-plain {
  list-style: none;
  padding-left: 0;
  font-size: clamp(1.4rem, 2.4vw, 2.8rem);
}
.bullets :deep(strong) {
  font-weight: 700;
}
.title-meta {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-top: 0.6rem;
  font-size: clamp(1rem, 1.5vw, 1.7rem);
  color: rgba(255, 255, 255, 0.85);
}
.presenter {
  font: inherit;
  font-weight: 600;
  color: #fff;
  background: transparent;
  border: 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.35);
  padding: 0.1rem 0;
  width: min(24ch, 100%);
  outline: none;
}
.presenter::placeholder {
  color: rgba(255, 255, 255, 0.5);
}
.presenter:focus {
  border-bottom-color: #fff;
}
.slide-link {
  align-self: flex-start;
  margin-top: 0.8rem;
  font-size: clamp(1.2rem, 1.6vw, 1.5rem);
  font-weight: 600;
  color: #fff;
  text-decoration: underline;
  text-underline-offset: 4px;
}
.note {
  margin: 0;
  font-size: clamp(0.95rem, 1.3vw, 1.3rem);
  color: rgba(255, 255, 255, 0.8);
  line-height: 1.45;
}

.footer {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  padding-top: 1rem;
}
.footer-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.counter {
  font-size: 0.95rem;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.75);
  font-variant-numeric: tabular-nums;
}
.nav {
  display: flex;
  gap: 0.5rem;
}
.round,
.pill {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 2.6rem;
  border-radius: 999px;
  border: 1.5px solid rgba(255, 255, 255, 0.55);
  background: transparent;
  color: #fff;
  cursor: pointer;
  font: inherit;
  transition: background 0.15s ease, border-color 0.15s ease;
}
.round {
  width: 2.6rem;
}
.pill {
  padding: 0 1.2rem;
  font-weight: 600;
}
.round:hover,
.pill:hover {
  background: rgba(255, 255, 255, 0.14);
  border-color: #fff;
}
.round:disabled {
  opacity: 0.35;
  cursor: default;
}
.hints {
  display: flex;
  gap: 1rem;
  font-size: 0.8rem;
  color: rgba(255, 255, 255, 0.5);
}
.progress {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 3px;
  background: rgba(255, 255, 255, 0.15);
}
.progress-fill {
  height: 100%;
  background: #ffb612;
  transition: width 0.3s ease;
}
@media (max-width: 1024px) {
  .deck {
    width: 100vw;
  }
}
</style>
