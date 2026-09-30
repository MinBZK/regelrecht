<script setup>
import { computed, onBeforeUnmount, onMounted, provide, ref, watch } from 'vue';
import Slide from './Slide.vue';
import * as api from './lib/api.js';
import { appendBlock, normalize, replaceRange, setYamlKey } from './lib/slideDoc.js';

// The deck: one folder, one slide per markdown file. Navigation and the footer
// follow the demo deck; editing writes every change straight back to the file
// it came from.

const props = defineProps({
  name: { type: String, required: true },
  index: { type: Number, default: 0 },
});
const emit = defineEmits(['go', 'close']);

const deck = ref(null);
const loadError = ref(null);
// For wet blocks: which deck a `bestand:` is relative to, and a counter that
// goes up when a law file may have changed, so the blocks fetch it again.
const wetVersion = ref(0);
provide('presenter:deck', { name: computed(() => props.name), wetVersion });
const editing = ref(false);
const status = ref('');
let statusTimer = null;

const total = computed(() => deck.value?.slides.length ?? 0);
const i = computed(() => Math.min(Math.max(props.index, 0), Math.max(total.value - 1, 0)));
const slide = computed(() => deck.value?.slides[i.value] ?? null);
const counter = computed(() => `${i.value + 1} / ${total.value}`);
const progress = computed(() => (total.value ? `${((i.value + 1) / total.value) * 100}%` : '0%'));

function flash(text) {
  status.value = text;
  clearTimeout(statusTimer);
  statusTimer = setTimeout(() => (status.value = ''), 2500);
}

async function load() {
  try {
    const d = await api.readDeck(props.name);
    // CRLF → LF, so the offsets of the blocks point into the text we hold. A
    // CRLF file is written back with LF on its first edit.
    for (const s of d.slides) s.content = normalize(s.content);
    d.metaRaw = normalize(d.metaRaw);
    deck.value = d;
    loadError.value = null;
    document.title = `${deck.value.meta.title ?? props.name} · Presentatie`;
  } catch (e) {
    loadError.value = e.message;
  }
}

// Writes run one after another. Each edit is computed from the content as it
// is once the previous write has landed, not as it was when the edit started;
// otherwise a second edit carries the first one's old mtime and is refused.
let queue = Promise.resolve();
function enqueue(task) {
  queue = queue.then(task, task);
  return queue;
}

/**
 * Write a slide file and keep the local copy in step, so no reload is needed.
 * `edit` maps the current content to the new content, or returns null when the
 * edit no longer applies (the text it was made against has changed).
 */
function writeSlide(s, edit) {
  return enqueue(async () => {
    const content = edit(s.content);
    if (content == null) {
      flash('Deze dia is intussen gewijzigd; opnieuw geladen, probeer het nog eens');
      return load();
    }
    try {
      const { mtime } = await api.writeFile(props.name, s.file, content, s.mtime);
      s.content = content;
      s.mtime = mtime;
      flash(`Opgeslagen in ${s.file}`);
    } catch (e) {
      flash(e.message);
      if (e.status === 409) await load();
    }
  });
}

function onReplace(block, text) {
  // The block's range is from the render it was clicked in. If the text at that
  // range is no longer the block's source, writing would overwrite something
  // else: refuse instead.
  writeSlide(slide.value, (content) =>
    block.start >= 0 && content.slice(block.start, block.end) === block.raw ? replaceRange(content, block.start, block.end, text) : null,
  );
}
function onAppend() {
  writeSlide(slide.value, (content) => appendBlock(content, 'Nieuwe alinea'));
}
function onMeta(key, value) {
  return enqueue(() => saveMeta(key, value));
}
async function saveMeta(key, value) {
  const d = deck.value;
  try {
    const content = setYamlKey(d.metaRaw, key, value);
    const { mtime } = await api.writeFile(props.name, 'deck.yaml', content, d.metaMtime);
    d.metaRaw = content;
    d.metaMtime = mtime;
    d.meta = { ...d.meta, [key]: value };
    flash('Opgeslagen in deck.yaml');
  } catch (e) {
    flash(e.message);
    if (e.status === 409) await load();
  }
}
async function newSlide() {
  try {
    const { file } = await api.createSlide(props.name, slide.value?.file ?? null);
    await load();
    const at = deck.value.slides.findIndex((s) => s.file === file);
    emit('go', at);
    flash(`Nieuwe dia: ${file}`);
  } catch (e) {
    flash(e.message);
  }
}

const go = (n) => emit('go', Math.min(Math.max(n, 0), total.value - 1));
function toggleFullscreen() {
  if (document.fullscreenElement) document.exitFullscreen();
  else document.documentElement.requestFullscreen?.();
}

function onKey(e) {
  const t = e.target;
  if (t instanceof HTMLElement && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  const k = e.key;
  if (k === 'ArrowRight' || k === 'PageDown' || k === ' ') go(i.value + 1);
  else if (k === 'ArrowLeft' || k === 'PageUp') go(i.value - 1);
  else if (k === 'Home') go(0);
  else if (k === 'End') go(total.value - 1);
  else if (k === 'f' || k === 'F') toggleFullscreen();
  else if (k === 'e' || k === 'E') editing.value = !editing.value;
  else if ((k === 'n' || k === 'N') && editing.value) newSlide();
  else if (k === 'Escape') {
    if (editing.value) editing.value = false;
    else emit('close');
  } else return;
  e.preventDefault();
}

// A change on disk (from an editor next to the browser) reloads the deck. Our
// own writes come through here too; they load the same text, so nothing moves.
function onDeckChanged({ deck: changed }) {
  if (changed !== props.name) return;
  // A law YAML in the deck folder may be what changed.
  api.clearWetCache();
  wetVersion.value++;
  load();
}
function onCorpusChanged() {
  api.clearWetCache();
  wetVersion.value++;
}

onMounted(() => {
  window.addEventListener('keydown', onKey);
  import.meta.hot?.on('presenter:deck-changed', onDeckChanged);
  import.meta.hot?.on('presenter:corpus-changed', onCorpusChanged);
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  import.meta.hot?.off?.('presenter:deck-changed', onDeckChanged);
  import.meta.hot?.off?.('presenter:corpus-changed', onCorpusChanged);
});
watch(() => props.name, load, { immediate: true });
</script>

<template>
  <div class="deck full" :class="{ editing }" role="region" aria-label="Presentatie">
    <div class="stage">
      <p v-if="loadError" class="note">{{ loadError }}</p>
      <p v-else-if="deck && !total" class="note">Deze map heeft nog geen dia's. Druk op E en dan N voor een eerste dia.</p>
      <Slide
        v-else-if="slide"
        :key="slide.file"
        :src="slide.content"
        :deck-meta="deck.meta"
        :editing="editing"
        @replace="onReplace"
        @append="onAppend"
        @meta="onMeta"
      />
    </div>

    <div class="footer">
      <div class="footer-text">
        <span class="counter">
          {{ counter }}<template v-if="editing && slide"> · {{ slide.file }}</template>
          <span v-if="status" class="status" role="status"> · {{ status }}</span>
        </span>
        <div class="hints">
          <span class="hint">
            <nldd-keyboard-shortcut size="sm" color="inherit" keys="←" always-visible></nldd-keyboard-shortcut>
            <nldd-keyboard-shortcut size="sm" color="inherit" keys="→" always-visible></nldd-keyboard-shortcut>
            bladeren
          </span>
          <span class="hint"><nldd-keyboard-shortcut size="sm" color="inherit" keys="E" always-visible></nldd-keyboard-shortcut> bewerken</span>
          <span v-if="editing" class="hint"><nldd-keyboard-shortcut size="sm" color="inherit" keys="N" always-visible></nldd-keyboard-shortcut> nieuwe dia</span>
          <span class="hint"><nldd-keyboard-shortcut size="sm" color="inherit" keys="F" always-visible></nldd-keyboard-shortcut> volledig scherm</span>
          <span class="hint"><nldd-keyboard-shortcut size="sm" color="inherit" keys="Esc" always-visible></nldd-keyboard-shortcut> {{ editing ? 'stop bewerken' : 'overzicht' }}</span>
        </div>
      </div>
      <nldd-button-bar>
        <nldd-button
          variant="inherit-tinted"
          start-icon="edit"
          :text="editing ? 'Klaar' : 'Bewerken'"
          @click="editing = !editing"
        ></nldd-button>
        <nldd-icon-button
          variant="inherit-tinted"
          icon="back"
          text="Vorige"
          tooltip-timing="never"
          :disabled="i === 0 || undefined"
          @click="go(i - 1)"
        ></nldd-icon-button>
        <nldd-icon-button
          variant="inherit-tinted"
          icon="forward"
          text="Volgende"
          tooltip-timing="never"
          :disabled="i >= total - 1 || undefined"
          @click="go(i + 1)"
        ></nldd-icon-button>
      </nldd-button-bar>
    </div>
    <div class="progress" aria-hidden="true"><div class="progress-fill" :style="{ width: progress }"></div></div>
  </div>
</template>
