<script setup>
import { computed } from 'vue';

const props = defineProps({
  node: { type: Object, default: null },
  // The prose sidecar body (Markdown) for the selected node, when one exists.
  prose: { type: String, default: null },
});
defineEmits(['close']);

// Minimal, dependency-free Markdown rendering for the prose overlay: the text is
// short "wat/waarom" narrative using only paragraphs and inline `**bold**`,
// `*italic*` and `` `code` ``. HTML is escaped first, so the repo-controlled
// prose renders as text, not markup.
function escapeHtml(s) {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

function inlineMarkdown(s) {
  return escapeHtml(s)
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*])\*([^*]+)\*/g, '$1<em>$2</em>')
    .replace(/`([^`]+)`/g, '<code>$1</code>');
}

const proseHtml = computed(() => {
  if (!props.prose) return '';
  return props.prose
    .split(/\n\s*\n/)
    .map((para) => para.trim())
    .filter(Boolean)
    .map((para) => `<p>${inlineMarkdown(para.replace(/\n/g, ' '))}</p>`)
    .join('');
});
</script>

<template>
  <aside v-if="node" class="detail-panel">
    <header class="detail-panel__head">
      <span class="detail-panel__badge" :class="`kind-${node.kind}`">{{ node.kind }}</span>
      <button type="button" class="detail-panel__close" aria-label="Sluiten" @click="$emit('close')">×</button>
    </header>

    <h2 class="detail-panel__name">{{ node.name }}</h2>

    <dl class="detail-panel__meta">
      <dt>Niveau</dt>
      <dd>{{ node.level }}</dd>

      <dt>Pad</dt>
      <dd><code>{{ node.path || '—' }}</code></dd>

      <dt>Id</dt>
      <dd><code class="detail-panel__id">{{ node.id }}</code></dd>
    </dl>

    <!-- Prose overlay: the hand-maintained "wat/waarom" for this node. -->
    <section v-if="proseHtml" class="detail-panel__prose">
      <h3 class="detail-panel__prose-title">Wat &amp; waarom</h3>
      <!-- eslint-disable-next-line vue/no-v-html -- content is repo-controlled and escaped above -->
      <div class="detail-panel__prose-body" v-html="proseHtml"></div>
    </section>

    <p v-if="node.doc" class="detail-panel__doc">{{ node.doc }}</p>
    <p v-else class="detail-panel__doc detail-panel__doc--empty">Geen doc-commentaar.</p>
  </aside>
</template>
