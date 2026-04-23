<script setup>
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { useEngine } from '../composables/useEngine.js';

const router = useRouter();
const { getDemoIndex, getProfile } = useEngine();

const profile = ref(null);
const laws = ref([]);
const error = ref(null);

onMounted(async () => {
  try {
    const [index, merijn] = await Promise.all([
      getDemoIndex(),
      getProfile('merijn'),
    ]);
    laws.value = index.laws;
    profile.value = merijn;
  } catch (e) {
    error.value = e?.message || String(e);
  }
});

function openLaw(law) {
  router.push({ name: 'law-detail', params: { lawId: law.id } });
}
</script>

<template>
  <nldd-page>
    <nldd-container padding="24">
      <nldd-title size="2"><h1>Waar heb ik recht op?</h1></nldd-title>
      <nldd-spacer size="4"></nldd-spacer>
      <p>Een kijkje in wet-als-code. Kies een wet en zie meteen de uitkomst voor onze persona.</p>
      <nldd-spacer size="16"></nldd-spacer>

      <div v-if="error" class="error">
        <nldd-title size="5"><h5>Oeps</h5></nldd-title>
        <p>{{ error }}</p>
      </div>

      <template v-else-if="profile">
        <nldd-title size="4"><h4>Jouw profiel</h4></nldd-title>
        <nldd-spacer size="4"></nldd-spacer>
        <nldd-list variant="box">
          <nldd-list-item size="md">
            <nldd-icon-cell size="32">
              <span class="avatar">{{ profile.avatar || '👤' }}</span>
            </nldd-icon-cell>
            <nldd-text-cell
              :text="profile.name"
              :supporting-text="`${profile.description} · BSN ${profile.bsn}`"
            ></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-spacer size="16"></nldd-spacer>
        <nldd-title size="4"><h4>Beschikbare wetten</h4></nldd-title>
        <nldd-spacer size="4"></nldd-spacer>
        <nldd-list variant="box">
          <nldd-list-item
            v-for="law in laws"
            :key="law.id"
            size="md"
            clickable
            @click="openLaw(law)"
          >
            <nldd-text-cell :text="law.label" :supporting-text="law.summary"></nldd-text-cell>
            <nldd-cell>
              <nldd-button text="Bekijk" @click.stop="openLaw(law)"></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
      </template>

      <nldd-spacer size="8" v-else></nldd-spacer>
      <p v-if="!profile && !error">Profiel en wetten laden…</p>
    </nldd-container>
  </nldd-page>
</template>

<style scoped>
.avatar { font-size: 32px; line-height: 1; }
.error { color: var(--primitives-color-danger-600, #b00020); }
</style>
