<script setup>
// Read-only docs renderer for /trajects/:id/docs pages.
//
// Re-uses the shared marked + DOMPurify pipeline from useArticleMarkdown so
// docs and law-text stay visually consistent (same list nesting, paragraph
// breaks, link styling). On top of that, ```mermaid``` code blocks are
// upgraded into rendered SVGs via a post-render pass on the DOM — kept here
// rather than in the shared pipeline because mermaid is a docs-only concern
// and the helper must stay generic.
//
// The post-render approach (replace <pre><code class="language-mermaid"> with
// <div class="mermaid"> + mermaid.run()) avoids touching marked's renderer
// or DOMPurify's allow-list, both of which are shared with other panes.

import { computed, nextTick, onMounted, ref, useTemplateRef, watch } from 'vue';
import mermaid from 'mermaid';

import { renderArticleHtml } from '../../composables/useArticleMarkdown.js';

const props = defineProps({
  /** Raw markdown source. Empty string renders nothing. */
  text: { type: String, default: '' },
});

const html = computed(() => renderArticleHtml(props.text));
const rootEl = useTemplateRef('rootEl');
const mermaidReady = ref(false);

onMounted(() => {
  // Initialize once per component lifecycle. Theme matches the editor's
  // NLDD light scheme; revisit when dark-mode lands repo-wide.
  mermaid.initialize({ startOnLoad: false, theme: 'neutral' });
  mermaidReady.value = true;
});

watch(
  [html, mermaidReady],
  async ([newHtml, ready]) => {
    if (!ready || !rootEl.value) return;
    await nextTick();
    await processMermaidBlocks(rootEl.value);
  },
  { immediate: true },
);

/** Find `<pre><code class="language-mermaid">…</code></pre>` blocks inside
 * the rendered content and replace each with a `<div class="mermaid">` that
 * mermaid.run() can hydrate into an SVG. Skips already-processed blocks
 * (idempotent across re-renders).
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
    await mermaid.run({ querySelector: '.docs-content .mermaid' });
  } catch (err) {
    // Mermaid throws on syntactically invalid diagrams. We surface that
    // inline rather than crashing the whole page; an editor reading the
    // analysis should see WHERE the diagram broke.
    console.warn('[docs] mermaid render error', err);
  }
}
</script>

<template>
  <article ref="rootEl" class="docs-content" v-html="html"></article>
</template>

<style scoped>
.docs-content {
  max-width: 80ch;
  line-height: 1.55;
}

.docs-content :deep(h1),
.docs-content :deep(h2),
.docs-content :deep(h3) {
  margin-top: 1.5em;
  margin-bottom: 0.5em;
}

.docs-content :deep(p),
.docs-content :deep(ul),
.docs-content :deep(ol) {
  margin: 0.6em 0;
}

.docs-content :deep(code) {
  background: var(--nldd-color-surface-subtle, #f5f5f5);
  padding: 0.1em 0.3em;
  border-radius: 2px;
  font-size: 0.92em;
}

.docs-content :deep(pre) {
  background: var(--nldd-color-surface-subtle, #f5f5f5);
  padding: 0.8em 1em;
  border-radius: 4px;
  overflow-x: auto;
}

.docs-content :deep(pre code) {
  background: transparent;
  padding: 0;
}

.docs-content :deep(blockquote) {
  margin: 0.6em 0;
  padding: 0.3em 1em;
  border-left: 3px solid var(--nldd-color-border, #ccc);
  color: var(--nldd-color-text-subtle, #555);
  font-style: italic;
}

.docs-content :deep(.mermaid) {
  display: flex;
  justify-content: center;
  margin: 1.2em 0;
}

.docs-content :deep(.mermaid svg) {
  max-width: 100%;
  height: auto;
}
</style>
