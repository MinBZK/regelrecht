<script setup>
import { onMounted, ref } from 'vue';
import ArchExplorer from './components/ArchExplorer.vue';

const model = ref(null);
const prose = ref({});
const error = ref(null);
const loading = ref(true);

async function load() {
  loading.value = true;
  error.value = null;
  try {
    const res = await fetch('api/model', { headers: { Accept: 'application/json' } });
    if (!res.ok) throw new Error(`HTTP ${res.status} bij ophalen van het model`);
    model.value = await res.json();
    // The prose sidecar is an optional overlay: a failure to load it must not
    // break the explorer, so it is fetched separately and defaults to empty.
    prose.value = await loadProse();
  } catch (e) {
    error.value = e?.message || String(e);
  } finally {
    loading.value = false;
  }
}

async function loadProse() {
  try {
    const res = await fetch('api/prose', { headers: { Accept: 'application/json' } });
    if (!res.ok) return {};
    return await res.json();
  } catch {
    return {};
  }
}

onMounted(load);
</script>

<template>
  <div class="app-root">
    <div v-if="loading" class="app-status">
      <div class="app-status__spinner"></div>
      <p>Model wordt gegenereerd uit de working tree… (eerste keer ~2 s)</p>
    </div>

    <div v-else-if="error" class="app-status app-status--error">
      <p>Kon het architectuurmodel niet laden.</p>
      <pre class="app-status__detail">{{ error }}</pre>
      <button type="button" class="arch-btn" @click="load">Opnieuw proberen</button>
    </div>

    <ArchExplorer v-else-if="model" :model="model" :prose="prose" />
  </div>
</template>
