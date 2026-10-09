<script setup>
import { computed } from 'vue';
import EditableBlock from './EditableBlock.vue';
import { bodyBlocks, splitFrontmatter } from './lib/slideDoc.js';

// One slide file, rendered like the demo deck's slide kinds
// (frontend-demo/src/presentation/PresentationDeck.vue):
//   title      # Titel, first paragraph is the subtitle, presenter + affiliation
//   statement  every paragraph is one big line
//   section    # Titel + plain lines
//   closing    same as section
//   content    # Titel + any markdown (the default)

const props = defineProps({
  src: { type: String, required: true },
  deckMeta: { type: Object, default: () => ({}) },
  editing: { type: Boolean, default: false },
});
const emit = defineEmits(['replace', 'append', 'meta']);

const fm = computed(() => splitFrontmatter(props.src).fm);
const kind = computed(() => fm.value.kind ?? 'content');
const all = computed(() => bodyBlocks(props.src));

const titleBlock = computed(() => {
  const first = all.value[0];
  return first?.token.type === 'heading' && first.token.depth === 1 ? first : null;
});
const rest = computed(() => all.value.slice(titleBlock.value ? 1 : 0));
// On a title slide the first paragraph under the title is the subtitle.
const subtitleBlock = computed(() => (kind.value === 'title' && rest.value[0]?.token.type === 'paragraph' ? rest.value[0] : null));
const bodyAfterSubtitle = computed(() => rest.value.slice(subtitleBlock.value ? 1 : 0));

const today = new Date().toLocaleDateString('nl-NL', { day: 'numeric', month: 'long', year: 'numeric' });
const dateLine = computed(() => fm.value.date ?? props.deckMeta.date ?? today);
const affiliation = computed(() => fm.value.footer ?? props.deckMeta.affiliation ?? '');

// Blocks are keyed by position, not by offset: a save that changes the length
// of one block must not remount (and close) an edit open in a later one.
function replace(block, text) {
  emit('replace', block, text);
}
function savePresenter(e) {
  const v = e.target?.value ?? '';
  if (v !== (props.deckMeta.presenter ?? '')) emit('meta', 'presenter', v);
}
</script>

<template>
  <div class="content" :class="`kind-${kind}`">
    <div class="content-main">
      <!-- Title slide -->
      <template v-if="kind === 'title'">
        <span class="overline">{{ dateLine }}</span>
        <h1 v-if="titleBlock" class="title title-hero">
          <EditableBlock :block="titleBlock" :editing="editing" inline @commit="replace(titleBlock, $event)" />
        </h1>
        <p v-if="subtitleBlock" class="lead lead-hero">
          <EditableBlock :block="subtitleBlock" :editing="editing" inline @commit="replace(subtitleBlock, $event)" />
        </p>
        <div class="md">
          <EditableBlock v-for="(b, j) in bodyAfterSubtitle" :key="j" :block="b" :editing="editing" @commit="replace(b, $event)" />
        </div>
        <div class="title-meta">
          <input class="presenter" :value="deckMeta.presenter ?? ''" placeholder="Naam presentator" aria-label="Naam presentator" @change="savePresenter" />
          <span v-if="affiliation" class="affiliation">{{ affiliation }}</span>
        </div>
      </template>

      <!-- Statement slide: a few big lines, one per paragraph -->
      <template v-else-if="kind === 'statement'">
        <span v-if="fm.overline" class="overline">{{ fm.overline }}</span>
        <div class="title statement" role="heading" aria-level="1">
          <EditableBlock v-for="(b, j) in all" :key="j" class="statement-line" :block="b" :editing="editing" inline @commit="replace(b, $event)" />
        </div>
      </template>

      <!-- Section / closing: a big title with a few plain lines -->
      <template v-else-if="kind === 'section' || kind === 'closing'">
        <span v-if="fm.overline" class="overline">{{ fm.overline }}</span>
        <h1 v-if="titleBlock" class="title title-hero">
          <EditableBlock :block="titleBlock" :editing="editing" inline @commit="replace(titleBlock, $event)" />
        </h1>
        <div class="md md-plain">
          <EditableBlock v-for="(b, j) in rest" :key="j" :block="b" :editing="editing" @commit="replace(b, $event)" />
        </div>
      </template>

      <!-- Content slide: title + markdown -->
      <template v-else>
        <span v-if="fm.overline" class="overline">{{ fm.overline }}</span>
        <h1 v-if="titleBlock" class="title">
          <EditableBlock :block="titleBlock" :editing="editing" inline @commit="replace(titleBlock, $event)" />
        </h1>
        <div class="md">
          <EditableBlock v-for="(b, j) in rest" :key="j" :block="b" :editing="editing" @commit="replace(b, $event)" />
        </div>
      </template>

      <nldd-button
        v-if="editing"
        class="add-block"
        variant="inherit-tinted"
        size="sm"
        start-icon="add"
        text="Blok toevoegen"
        @click="emit('append')"
      ></nldd-button>
    </div>
    <div v-if="fm.note" class="content-foot">
      <p class="note">{{ fm.note }}</p>
    </div>
  </div>
</template>
