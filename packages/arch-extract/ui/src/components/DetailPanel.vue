<script setup>
defineProps({
  node: { type: Object, default: null },
});
defineEmits(['close']);
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

    <p v-if="node.doc" class="detail-panel__doc">{{ node.doc }}</p>
    <p v-else class="detail-panel__doc detail-panel__doc--empty">Geen doc-commentaar.</p>
  </aside>
</template>
