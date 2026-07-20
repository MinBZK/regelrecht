<script setup>
import { computed, ref, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import SearchPopover from './components/SearchPopover.vue';
import TrajectCreateForm from './components/TrajectCreateForm.vue';
import { createTraject } from './composables/useTrajects.js';
import { registerSearchPopover } from './composables/useAppChrome.js';

// Nieuw-traject-pagina — bereikbaar vanaf de trajectkeuze-pagina
// (/editor). Gebruikt hetzelfde gedeelde formulier als de
// TrajectMenu-sheet; na het aanmaken ga je direct de editor in op het
// nieuwe traject, met een eventueel meegekregen wet (query
// `law`/`article`) voorgeselecteerd.
//
// Content-only: de chrome komt uit de persistente AppShell; de terugknop
// naar de keuzepagina zit in de page-header.
const route = useRoute();
const router = useRouter();

const searchPopoverRef = ref(null);
registerSearchPopover(searchPopoverRef);

const formEl = ref(null);
const createBusy = ref(false);
const createError = ref(null);

watchEffect(() => {
  document.title = 'Nieuw traject · RegelRecht';
});

// Terug naar de keuzepagina, met behoud van een meegekregen wet.
const backTarget = computed(() => ({ name: 'editor', query: route.query }));

// Deze pagina heeft zelf geen wetweergave — een zoekresultaat opent in de
// bibliotheek.
function openLawFromSearch(lawId) {
  router.push({ name: 'library', params: { lawId } });
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
      name: 'editor-traject',
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
  <nldd-page sticky-header>
    <nldd-top-title-bar
      slot="header"
      text="Nieuw traject"
      :back-text="createBusy ? undefined : 'Kies een traject'"
      collapse-anchor="nieuw-traject-titel"
      @back="router.push(backTarget)"
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

  <SearchPopover
    ref="searchPopoverRef"
    @select-law="openLawFromSearch"
  />
</template>
