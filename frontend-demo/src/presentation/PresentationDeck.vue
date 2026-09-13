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
      <!-- Het podium: de tekstkolom van de dia. Op het hele scherm is dat een
           gecentreerde kolom van hooguit 1600px, in de rail de hele kolom. In
           beide gevallen is dit de container waar de typografie zich op meet,
           zodat één ladder voor allebei volstaat. -->
      <div class="stage">
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
              <span v-for="(line, j) in p.current.value.lines" :key="j" class="statement-line" v-html="emphasize(line)"></span>
            </h1>
          </template>

          <!-- Section or closing slide: a big title with a few plain lines -->
          <template v-else-if="p.current.value.kind === 'closing' || p.current.value.kind === 'section'">
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
  /* De maat volgt de BREEDTE van het vlak waarin de tekst staat (`1cqw`), niet
     de kortste zijde. Met `cqmin` won op elk normaal venster de hoogte, en die
     zegt niets over hoe groot een letter mag zijn: op een breed scherm stond de
     titel op 26px in een vlak van 720px, met de rest van de dia leeg. Een dia
     vult zijn regel; de hoogte begrenst, maar is geen maatstaf. */
  container-type: inline-size;
  /* Twee rollen, twee schalen. Op het hele scherm is de dia het beeld en mag
     de titel de regel vullen; in de rail is hij een bijschrift naast de demo,
     en daar is diezelfde maat schreeuwerig. `--scale` is de enige knop: de
     rail zet hem lager, de rest van de typografie hieronder is één ladder. */
  /* De rail is geen verkleinde dia maar een leespaneel naast de demo. Eén
     procent van een kolom van 630px is 6,3px, en de dia-ladder maakte daar
     titels van 28px en opsommingen van 13px van: kleiner dan de tekst van de
     demo ernaast. De ondergrens tilt de hele ladder in één keer op, zodat de
     verhoudingen blijven kloppen en alleen het formaat leesbaar wordt. */
  --scale: 1;
  --slide-floor: 11px;
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
/* Een dia op het hele scherm is geen vlak met marges eromheen maar een
   gecentreerde tekstkolom met een begrensde regellengte. Zonder die grens
   bepaalde het vensterformaat de regel: op 2000px bleef er na de marges 1680px
   over voor een regel van 26px, en de dia was vooral leeg blauw. */
.deck.full {
  width: 100vw;
  padding: 2vh 0 0;
  /* De deck zelf meet niets meer: het podium binnenin is de container. */
  container-type: normal;
}
.deck.full .stage {
  /* Het podium vult de vrije ruimte; het dwingt géén 16:9 af. Dat deed het
     eerder wel, en dan bepaalde de vorm van het vlak of de tekst paste: de
     titeldia is hoger dan 16:9 bij deze lettergrootte, en "Nederlandse
     Digitale Dienst" werd er onderaan afgesneden.
     De maat komt nu van de breedte (`--slide-measure`), begrensd op een
     leesbare regellengte, en de hoogte is vrij. Zo bepaalt de inhoud de
     hoogte en de regellengte de typografie, in plaats van andersom. */
  --scale: 1;
  --slide-floor: 0px;
  container-type: inline-size;
  flex: 1 1 auto;
  min-height: 0;
  width: min(100%, 1600px);
  margin: 0 auto;
  padding: 3vh clamp(2rem, 5vw, 6rem);
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}
/* De voettekst en de voortgangsbalk horen bij de deck, niet bij de dia: ze
   staan naast het verhaal en mogen het podium niet uit verhouding duwen. */
.deck.full .footer {
  padding-inline: clamp(1.5rem, 4vw, 4rem);
}

/* In de rail is het podium gewoon de kolom: geen 16:9, want daar staat de dia
   naast de demo en is hij een bijschrift, geen beeld. */
.stage {
  /* `--slide-unit` staat hier en niet op de deck: `1cqw` moet zich meten aan
     het podium, en `--scale` moet de waarde van dít element zijn. Op de deck
     werd de rail-schaal van 0.62 ook op een volledige dia toegepast. */
  --slide-unit: max(var(--slide-floor, 0px), calc(1cqw * var(--scale)));
  container-type: inline-size;
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* A slide never scrolls. Scrolling hides the bottom of an argument behind a
 * gesture nobody makes while presenting, and on a projector the speaker cannot
 * see that there is more. De typografie schaalt in plaats daarvan mee met de
 * breedte van het podium (`--slide-unit`, een percentage daarvan), met een
 * ondergrens in de rail zodat het bijschrift daar niet kleiner wordt dan de
 * tekst van de demo ernaast. */
.content {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}
.content-main {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: max(0.5rem, calc(1.6 * var(--slide-unit)));
}
.content-foot {
  flex: 0 0 auto;
  padding-top: 1.25rem;
  margin-top: 1.5rem;
  border-top: 1px solid rgba(255, 255, 255, 0.18);
}

/* Eén typografische ladder, in stappen van `--slide-unit` (een percentage van
   de podiumbreedte maal `--scale`). Geen `clamp()`-plafonds meer: die kapten
   op een beamer juist af wat daar goed was, en de vloer is overbodig zolang de
   dia een podium met vaste verhoudingen is dat zelf niet kleiner wordt dan het
   venster. Alleen een ondergrens blijft, voor een heel smal venster. */
.overline {
  font-size: max(0.72rem, calc(1.6 * var(--slide-unit)));
  font-weight: 600;
  letter-spacing: 0.02em;
  color: rgba(255, 255, 255, 0.72);
}
.title {
  font-family: 'RijksoverheidSerif', Georgia, serif;
  font-weight: 700;
  font-size: max(1.2rem, calc(5.2 * var(--slide-unit)));
  line-height: 1.06;
  letter-spacing: -0.015em;
  text-wrap: balance;
  margin: 0;
  color: #fff;
}
.title-hero {
  font-size: max(1.6rem, calc(8.5 * var(--slide-unit)));
}
.statement {
  font-size: max(1.1rem, calc(5.2 * var(--slide-unit)));
  line-height: 1.18;
  font-weight: 400;
  letter-spacing: -0.01em;
}
/* One authored line per block; a line that still has to wrap balances its
 * halves instead of leaving one word behind. */
.statement-line {
  display: block;
  text-wrap: balance;
}
.statement :deep(strong) {
  font-weight: 700;
}
.lead {
  font-size: max(0.85rem, calc(2.6 * var(--slide-unit)));
  line-height: 1.4;
  margin: 0;
  font-weight: 500;
  text-wrap: pretty;
}
.lead-hero {
  font-family: 'RijksoverheidSerif', Georgia, serif;
  font-style: italic;
  font-weight: 400;
  font-size: max(1rem, calc(3.6 * var(--slide-unit)));
  color: rgba(255, 255, 255, 0.9);
}
.bullets {
  font-size: max(0.8rem, calc(2.2 * var(--slide-unit)));
  line-height: 1.45;
  margin: 0;
  padding-left: 1.3em;
  display: flex;
  flex-direction: column;
  gap: calc(0.9 * var(--slide-unit));
  color: rgba(255, 255, 255, 0.96);
  text-wrap: pretty;
}
.bullets li::marker {
  color: rgba(255, 255, 255, 0.7);
}
.bullets-plain {
  list-style: none;
  padding-left: 0;
  font-size: max(0.9rem, calc(3 * var(--slide-unit)));
}
.bullets :deep(strong) {
  font-weight: 700;
}
.title-meta {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-top: calc(1.5 * var(--slide-unit));
  font-size: max(0.9rem, calc(1.9 * var(--slide-unit)));
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
  margin-top: calc(1.5 * var(--slide-unit));
  font-size: max(0.95rem, calc(2 * var(--slide-unit)));
  font-weight: 600;
  color: #fff;
  text-decoration: underline;
  text-underline-offset: 4px;
}
.note {
  margin: 0;
  font-size: max(0.85rem, calc(1.6 * var(--slide-unit)));
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
