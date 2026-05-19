<script setup>
// Read-only docs renderer.
//
// Wraps the shared marked + DOMPurify pipeline (`useArticleMarkdown`,
// reused for visual parity with ArticleText / AnnotatedText) into an
// `<nldd-rich-text>` so the typography, link styling, list spacing,
// and code-block treatment match the rest of the editor without any
// custom CSS of our own.
//
// On top, ```mermaid``` fenced code blocks are upgraded into rendered
// SVGs via a post-render DOM pass — `nldd-rich-text` keeps its slot
// content in the light DOM, so `querySelectorAll` from the ref works
// without shadow-DOM piercing (same approach AnnotatedText uses for
// note highlights).

import { computed, nextTick, onMounted, ref, useTemplateRef, watch } from 'vue';
import mermaid from 'mermaid';

import { renderArticleHtml } from '../../composables/useArticleMarkdown.js';

const props = defineProps({
  /** Raw markdown source. Empty string renders an empty rich-text block. */
  text: { type: String, default: '' },
});

const html = computed(() => renderArticleHtml(props.text));
const richTextEl = useTemplateRef('richTextEl');
const mermaidReady = ref(false);

onMounted(() => {
  // Initialize once per component lifecycle. Neutral theme reads well in
  // both light and dark NLDD palettes; revisit when @nldd/design-system
  // exposes a "current theme" signal we can subscribe to.
  mermaid.initialize({ startOnLoad: false, theme: 'neutral' });
  mermaidReady.value = true;
});

watch(
  [html, mermaidReady],
  async ([, ready]) => {
    if (!ready || !richTextEl.value) return;
    await nextTick();
    await processMermaidBlocks(richTextEl.value);
  },
  { immediate: true },
);

/** Find `<pre><code class="language-mermaid">…</code></pre>` blocks inside
 * the rendered rich-text and replace each with a `<div class="mermaid">`
 * that mermaid.run() can hydrate into an SVG. Idempotent across re-renders.
 */
async function processMermaidBlocks(root) {
  const blocks = root.querySelectorAll('pre > code.language-mermaid');
  if (blocks.length === 0) return;

  for (const code of blocks) {
    const pre = code.parentElement;
    if (!pre || pre.dataset.mermaidReplaced === 'true') continue;
    const source = code.textContent || '';
    const div = document.createElement('div');
    div.className = 'mermaid';
    div.textContent = source;
    pre.replaceWith(div);
  }

  try {
    await mermaid.run({ querySelector: '.docs-content-rich .mermaid' });
  } catch (err) {
    // Mermaid throws on syntactically invalid diagrams. Surface inline
    // rather than crashing the page — an analysis reader should see WHERE
    // the diagram broke, not a blank pane.
    console.warn('[docs] mermaid render error', err);
  }
}
</script>

<template>
  <nldd-rich-text ref="richTextEl" class="docs-content-rich" v-html="html"></nldd-rich-text>
</template>

<style scoped>
/* Center the rendered mermaid SVGs inside the rich-text slot. Everything
 * else (headings, paragraphs, lists, code, blockquote) inherits from
 * nldd-rich-text and intentionally has no local override. */
.docs-content-rich :deep(.mermaid) {
  display: flex;
  justify-content: center;
  margin: 1.2em 0;
}

.docs-content-rich :deep(.mermaid svg) {
  max-width: 100%;
  height: auto;
}
</style>
