<script setup>
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useDemo } from '../store/demoStore.js';

// The opening deck: a handful of statement slides driven by the keyboard.
// Arrow right / space / page down advance; on the last slide the next press
// hands over to the Wetten tab, the scripted start of the live demo.

const router = useRouter();
const { corpus, state } = useDemo();
const slides = computed(() => corpus.value?.config?.slides ?? []);
const current = ref(0);

const today = new Date().toLocaleDateString('nl-NL', { day: 'numeric', month: 'long', year: 'numeric' });

function next() {
  if (current.value < slides.value.length - 1) current.value += 1;
  else router.push('/wetten');
}
function prev() {
  if (current.value > 0) current.value -= 1;
}

function onKey(e) {
  if (e.target?.closest?.('input, textarea, [contenteditable]')) return;
  switch (e.key) {
    case 'ArrowRight':
    case ' ':
    case 'PageDown':
      e.preventDefault();
      next();
      break;
    case 'ArrowLeft':
    case 'PageUp':
      e.preventDefault();
      prev();
      break;
    case 'Home':
      current.value = 0;
      break;
    case 'End':
      current.value = slides.value.length - 1;
      break;
    default:
  }
}

let listening = false;
function listen() {
  if (!listening) {
    window.addEventListener('keydown', onKey);
    listening = true;
  }
}
function unlisten() {
  if (listening) {
    window.removeEventListener('keydown', onKey);
    listening = false;
  }
}
onMounted(listen);
onActivated(listen);
onDeactivated(unlisten);
onUnmounted(unlisten);

// Presenter name on the title slide, editable in place and remembered.
const editingName = ref(false);
function saveName(e) {
  state.presenterName = e.target?.value ?? '';
  editingName.value = false;
}

/** `**bold**` markup in a statement line → HTML with <strong>. */
function emphasize(line) {
  const escaped = line.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
  return escaped.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
}
</script>

<template>
  <nldd-page>
    <nldd-full-bleed-section height="100%" padding-block="0">
      <div class="slide-stage" @click="next">
        <div v-for="(slide, i) in slides" :key="i" v-show="i === current" class="slide">
          <template v-if="slide.kind === 'title'">
            <nldd-title size="1">
              <span slot="overline">{{ today }}</span>
              <h1>{{ slide.title }}</h1>
              <span slot="subtitle">{{ slide.subtitle }}</span>
            </nldd-title>
            <nldd-spacer size="32"></nldd-spacer>
            <nldd-rich-text centered>
              <p v-if="!editingName" @dblclick.stop="editingName = true" title="Dubbelklik om te wijzigen">
                {{ state.presenterName || 'Presentator' }}
              </p>
              <p v-else>
                <nldd-text-field
                  :value="state.presenterName"
                  placeholder="Naam presentator"
                  accessible-label="Naam presentator"
                  @click.stop
                  @change="saveName"
                  @keydown.enter="saveName"
                ></nldd-text-field>
              </p>
              <p><small>{{ slide.footer }}</small></p>
            </nldd-rich-text>
          </template>
          <template v-else>
            <nldd-title size="2">
              <span slot="overline">{{ slide.overline }}</span>
              <h2>
                <template v-for="(line, j) in slide.lines" :key="j">
                  <span v-html="emphasize(line)"></span><br v-if="j < slide.lines.length - 1" />
                </template>
              </h2>
            </nldd-title>
          </template>
        </div>
      </div>
    </nldd-full-bleed-section>
    <nldd-container slot="footer" padding="12" layout="row" horizontal-alignment="center" vertical-alignment="center" gap="12">
      <nldd-icon-button size="sm" variant="neutral-transparent" icon="chevron-left" text="Vorige" :disabled="current === 0 || undefined" @click="prev"></nldd-icon-button>
      <nldd-tag size="sm" :text="`${current + 1} / ${slides.length}`"></nldd-tag>
      <nldd-icon-button size="sm" variant="neutral-transparent" :icon="current === slides.length - 1 ? 'arrow-right' : 'chevron-right'" :text="current === slides.length - 1 ? 'Naar de wetten' : 'Volgende'" @click="next"></nldd-icon-button>
    </nldd-container>
  </nldd-page>
</template>
