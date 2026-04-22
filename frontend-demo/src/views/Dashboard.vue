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
  <ndd-page>
    <ndd-container padding="24">
      <ndd-title size="2"><h1>Waar heb ik recht op?</h1></ndd-title>
      <ndd-spacer size="4"></ndd-spacer>
      <p>Een kijkje in wet-als-code. Kies een wet en zie meteen de uitkomst voor onze persona.</p>
      <ndd-spacer size="16"></ndd-spacer>

      <div v-if="error" class="error">
        <ndd-title size="5"><h5>Oeps</h5></ndd-title>
        <p>{{ error }}</p>
      </div>

      <template v-else-if="profile">
        <ndd-title size="4"><h4>Jouw profiel</h4></ndd-title>
        <ndd-spacer size="4"></ndd-spacer>
        <ndd-container padding="16" class="profile-card">
          <div class="profile-row">
            <div class="avatar">{{ profile.avatar || '👤' }}</div>
            <div>
              <strong>{{ profile.name }}</strong><br />
              <small>{{ profile.description }}</small><br />
              <small>BSN {{ profile.bsn }}</small>
            </div>
          </div>
        </ndd-container>

        <ndd-spacer size="16"></ndd-spacer>
        <ndd-title size="4"><h4>Beschikbare wetten</h4></ndd-title>
        <ndd-spacer size="4"></ndd-spacer>
        <ndd-list variant="box">
          <ndd-list-item
            v-for="law in laws"
            :key="law.id"
            size="md"
            clickable
            @click="openLaw(law)"
          >
            <ndd-text-cell :text="law.label" :supporting-text="law.summary"></ndd-text-cell>
            <ndd-cell>
              <ndd-button text="Bekijk" @click.stop="openLaw(law)"></ndd-button>
            </ndd-cell>
          </ndd-list-item>
        </ndd-list>
      </template>

      <ndd-spacer size="8" v-else></ndd-spacer>
      <p v-if="!profile && !error">Profiel en wetten laden…</p>
    </ndd-container>
  </ndd-page>
</template>

<style scoped>
.profile-card { border: 1px solid var(--ndd-color-neutral-200, #e5e5e5); border-radius: 8px; }
.profile-row { display: flex; align-items: center; gap: 16px; }
.avatar { font-size: 48px; line-height: 1; }
.error { color: var(--ndd-color-red-600, #b00020); }
</style>
