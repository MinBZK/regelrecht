<script setup>
import { ref, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import TrajectCreateForm from './components/TrajectCreateForm.vue';
import { createTraject } from './composables/useTrajects.js';

// Nieuw-traject-pagina — bereikbaar vanaf de trajectkeuze-pagina. Gebruikt
// hetzelfde gedeelde formulier als de TrajectMenu-sheet; na het aanmaken ga je
// direct de editor (of traject-scoped bibliotheek) in op het nieuwe traject,
// met een eventueel meegekregen wet (query `law`/`article`) voorgeselecteerd.
//
// Top-level route (geen AppShell-child): geen app-chrome. De pagina draagt z'n
// eigen top-title-bar met terugknop naar de trajectkeuze.
const route = useRoute();
const router = useRouter();

const formEl = ref(null);
const createBusy = ref(false);
const createError = ref(null);

watchEffect(() => {
  document.title = 'Nieuw traject · RegelRecht';
});

// Terug naar de chooser, met behoud van sectie + meegekregen wet.
function goBack() {
  router.push({ name: 'trajecten', query: route.query });
}

async function submitCreate() {
  createError.value = null;
  const { payload, error } = formEl.value.buildPayload();
  if (error) {
    createError.value = error;
    return;
  }
  createBusy.value = true;
  try {
    const created = await createTraject(payload);
    // Mirror the TrajectMenu guard: the backend's `create` handler always
    // calls `fill_ref()`, but a half-filled TrajectSummary would otherwise
    // navigate to `/editor/undefined/...` and silently no-op.
    if (!created.ref) {
      console.warn('TrajectCreateView: created traject has no ref', created);
      createError.value =
        'Traject aangemaakt maar kon niet worden geopend. Ga terug en selecteer het.';
      return;
    }
    await router.push({
      name: route.query.sectie === 'library' ? 'library-traject' : 'editor-traject',
      params: {
        trajectRef: created.ref,
        lawId: route.query.law || undefined,
        articleNumber: route.query.article || undefined,
      },
    });
  } catch (e) {
    createError.value = e.message || 'Aanmaken mislukt';
  } finally {
    createBusy.value = false;
  }
}
</script>

<template>
  <nldd-app-view>
    <nldd-page sticky-header>
      <nldd-top-title-bar
        slot="header"
        text="Nieuw traject"
        :back-text="createBusy ? undefined : 'Trajecten'"
        collapse-anchor="nieuw-traject-titel"
        @back="goBack"
      ></nldd-top-title-bar>

      <nldd-simple-section width="800px">
        <nldd-title id="nieuw-traject-titel" size="3"><h3>Nieuw traject</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>
        <TrajectCreateForm ref="formEl"
          :busy="createBusy"
          :error="createError"
          @submit="submitCreate"
        />
      </nldd-simple-section>
    </nldd-page>
  </nldd-app-view>
</template>
