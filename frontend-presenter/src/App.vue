<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue';
import DeckView from './DeckView.vue';
import { listDecks } from './lib/api.js';

// Two screens, addressed by the hash so a reload stays on the same slide:
//   #/                       the list of decks (folders)
//   #/deck/<naam>/<nummer>   a slide, numbered from 1 like the counter

const route = ref(parse());
const decks = ref([]);
const error = ref(null);

function parse() {
  const m = location.hash.match(/^#\/deck\/([^/]+)(?:\/(\d+))?/);
  return m ? { deck: decodeURIComponent(m[1]), index: Math.max(Number(m[2] ?? 1) - 1, 0) } : { deck: null, index: 0 };
}
function onHash() {
  route.value = parse();
  if (!route.value.deck) refresh();
}
function open(deck, index = 0) {
  location.hash = `#/deck/${encodeURIComponent(deck)}/${index + 1}`;
}
function home() {
  location.hash = '#/';
  document.title = 'Presentatie · RegelRecht';
}
async function refresh() {
  try {
    decks.value = await listDecks();
    error.value = null;
  } catch (e) {
    error.value = e.message;
  }
}

onMounted(() => {
  window.addEventListener('hashchange', onHash);
  if (!route.value.deck) refresh();
});
onBeforeUnmount(() => window.removeEventListener('hashchange', onHash));
</script>

<template>
  <DeckView v-if="route.deck" :name="route.deck" :index="route.index" @go="open(route.deck, $event)" @close="home" />
  <nldd-page v-else>
    <nldd-simple-section width="720px">
      <nldd-title slot="header" size="2">
        <span slot="overline">Presentatie</span>
        <h1>Decks</h1>
        <span slot="subtitle">Elke map is een presentatie, elk markdown-bestand erin een dia.</span>
      </nldd-title>
      <nldd-rich-text spacing="tight">
        <p v-if="error">{{ error }}</p>
        <p v-else-if="!decks.length">Nog geen decks. Maak een map aan in de decks-map met een <code>01-titel.md</code> erin.</p>
      </nldd-rich-text>
      <nldd-list v-if="decks.length" variant="box-base" accessible-label="Decks">
        <nldd-list-item v-for="d in decks" :key="d.name" size="sm" button @click="open(d.name)">
          <nldd-text-cell size="sm" :text="d.title" :supporting-text="`${d.name} · ${d.slides} dia's`"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-simple-section>
  </nldd-page>
</template>
