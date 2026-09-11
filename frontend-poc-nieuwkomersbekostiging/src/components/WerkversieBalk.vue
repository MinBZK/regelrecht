<template>
  <div v-if="ready" class="wv" :class="{ 'wv-variant': !!werkversie, 'wv-bewerkt': changeCount > 0 }">
    <div class="wv-links">
      <span class="wv-label">Werkversie</span>
      <nldd-dropdown :key="`wv:${werkversie}:${variants.length}:${herteken}`" size="sm" width="360px" @change="kies($event.detail?.value ?? '')">
        <select :value="werkversie ?? ''" aria-label="Kies de werkversie: wat je bewerkt">
          <option value="">Huidig recht (basis)</option>
          <option v-for="v in variants" :key="v.id" :value="v.id">{{ kortTitel(v) }}</option>
        </select>
      </nldd-dropdown>
      <span class="wv-status">
        <template v-if="changeCount === 0">ongewijzigd</template>
        <template v-else>{{ changeCount }} {{ changeCount === 1 ? 'document bewerkt' : 'documenten bewerkt' }}</template>
      </span>
    </div>
    <div class="wv-rechts">
      <nldd-button size="sm" text="Bekijk wijzigingen" start-icon="document" variant="neutral-transparent" :disabled="!werkversie && changeCount === 0 ? true : undefined" @click="diffOpen = true"></nldd-button>
      <nldd-button size="sm" text="Terugzetten" start-icon="undo" variant="neutral-transparent" :disabled="changeCount === 0 ? true : undefined" @click="resetChanges"></nldd-button>
      <nldd-button size="sm" text="Bewaar als variant" start-icon="save" variant="secondary" :disabled="changeCount === 0 ? true : undefined" @click="opslaanOpen = !opslaanOpen"></nldd-button>
      <!-- "Terugzetten" hierboven maakt bewerkingen in de wet ongedaan; deze
           knop wist wat de browser onthoudt (kolommen, gekozen casus of
           school, populatie-instellingen) en begint de demo schoon. -->
      <nldd-button
        v-if="ietsBewaard"
        size="sm"
        text="Begin opnieuw"
        start-icon="undo"
        variant="neutral-transparent"
        @click="opnieuwOpen = true"
      ></nldd-button>
    </div>

    <nldd-banner v-if="opnieuwOpen" variant="warning" class="wv-breed">
      Hiermee vergeet de demo je keuzes: de gekozen kolommen, de werkversie, welke casus en school je bekijkt en de
      instellingen van de populatie. De wet zelf en de opgeslagen varianten blijven staan.
      <span class="wv-knoppen">
        <nldd-button size="sm" text="Wis en begin opnieuw" variant="primary" @click="wisStand"></nldd-button>
        <nldd-button size="sm" text="Annuleren" variant="neutral-transparent" @click="opnieuwOpen = false"></nldd-button>
      </span>
    </nldd-banner>

    <nldd-banner v-if="wisselNaar !== null" variant="warning" class="wv-breed">
      Je hebt {{ changeCount }} bewerkte {{ changeCount === 1 ? 'document' : 'documenten' }} in {{ werkversieLabel }}. Wisselen gooit die weg.
      <span class="wv-knoppen">
        <nldd-button size="sm" text="Toch wisselen" variant="primary" @click="wisselBevestigd"></nldd-button>
        <nldd-button size="sm" text="Annuleren" variant="neutral-transparent" @click="annuleerWissel"></nldd-button>
      </span>
    </nldd-banner>

    <div v-if="opslaanOpen" class="wv-breed wv-opslaan">
      <p class="wv-uitleg">
        Maakt een git-branch <code>variant/&lt;sleutel&gt;</code> met de bewerkte documenten
        {{ werkversie ? `(de branch van ${werkversieLabel} plus jouw bewerkingen)` : '' }}
        en zet die als nieuwe kolom klaar. De branch blijft lokaal tot je hem pusht.
      </p>
      <div class="wv-velden">
        <nldd-form-field label="Titel" supporting-label="wordt de kolomkop, bijvoorbeeld: drempel naar drie leerlingen">
          <nldd-text-field size="sm" :value="titel" @input="titel = $event.detail?.value ?? titel"></nldd-text-field>
        </nldd-form-field>
        <nldd-form-field label="Sleutel" supporting-label="branchnaam: variant/<sleutel>">
          <nldd-text-field size="sm" :value="sleutel" @input="sleutelHandmatig = true; sleutel = $event.detail?.value ?? sleutel"></nldd-text-field>
        </nldd-form-field>
      </div>
      <div class="wv-knoppen">
        <nldd-button size="sm" text="Bewaar" variant="primary" :disabled="!titel.trim() || !sleutelOk || bezig ? true : undefined" @click="opslaan"></nldd-button>
        <nldd-button size="sm" text="Annuleren" variant="neutral-transparent" @click="opslaanOpen = false"></nldd-button>
        <span v-if="bezig" class="wv-status">bezig…</span>
      </div>
    </div>
    <nldd-banner v-if="melding" :variant="meldingSoort" class="wv-breed" dismissible @dismiss="melding = ''">{{ melding }}</nldd-banner>
  </div>
  <diff-sheet :open="diffOpen" @close="diffOpen = false" />
</template>

<!--
  De werkversiebalk staat op elke pagina onder de navigatie. Er is precies één
  werkversie (huidig recht of één variant); alles wat bewerkt werkt daarop.
-->

<script setup>
import { ref, computed, watch } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { b } from '../basePad.js';
import { useLawStore, kortTitel } from '../engine/lawStore.js';
import { useSimulation } from '../composables/useSimulation.js';
import DiffSheet from './beleid/DiffSheet.vue';
import { wisStand, heeftBewaardeStand } from '../composables/useBewaardeStand.js';

const { ready } = useEngine();
const { variants, werkversie, werkversieLabel, changeCount, setWerkversie, resetChanges, editedFilesForSave, reloadVariants } = useLawStore();
const { selectedVariants } = useSimulation();

const diffOpen = ref(false);
const wisselNaar = ref(null); // '' = huidig recht, anders variant-id; null = geen wissel gevraagd
const herteken = ref(0); // bumpen zet de keuzelijst terug op de werkversie na annuleren
const opnieuwOpen = ref(false);
const ietsBewaard = heeftBewaardeStand();
const opslaanOpen = ref(false);
const titel = ref('');
const sleutel = ref('');
const bezig = ref(false);
const melding = ref('');
const meldingSoort = ref('accent');

const sleutelOk = computed(() => /^[a-z0-9][a-z0-9-]{1,40}$/.test(sleutel.value));

const sleutelHandmatig = ref(false); // true zodra de gebruiker de sleutel zelf aanpaste
watch(titel, (t) => {
  // Sleutel afleiden van de titel zolang de gebruiker hem niet zelf aanpaste.
  const auto = t.toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40);
  if (!sleutelHandmatig.value) sleutel.value = auto;
});

function kies(value) {
  const doel = value === '' ? null : value;
  if (doel === (werkversie.value ?? null)) return;
  if (changeCount.value > 0) {
    wisselNaar.value = value;
    return;
  }
  doeWissel(doel);
}

async function doeWissel(doel) {
  await setWerkversie(doel);
  if (doel && !selectedVariants.value.includes(doel)) selectedVariants.value = [...selectedVariants.value, doel].slice(-3);
}

function annuleerWissel() {
  wisselNaar.value = null;
  herteken.value++;
}

async function wisselBevestigd() {
  const value = wisselNaar.value;
  wisselNaar.value = null;
  await doeWissel(value === '' ? null : value);
}

async function opslaan() {
  bezig.value = true;
  melding.value = '';
  try {
    const bestanden = editedFilesForSave();
    const res = await fetch(b('/api/variant'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sleutel: sleutel.value, titel: titel.value.trim(), bestanden, basis: werkversie.value }),
    });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body.fout ?? `Backend gaf ${res.status}`);
    const id = body.id ?? sleutel.value;
    const branch = body.branch ?? `variant/${id}`;
    opslaanOpen.value = false;
    titel.value = '';
    sleutel.value = '';
    sleutelHandmatig.value = false;
    try {
      await reloadVariants();
      await setWerkversie(id);
      if (!selectedVariants.value.includes(id)) selectedVariants.value = [...selectedVariants.value, id].slice(-3);
      meldingSoort.value = 'success';
      melding.value = `Bewaard als branch ${branch}; de nieuwe variant is nu de werkversie en staat als kolom klaar.`;
    } catch (e) {
      meldingSoort.value = 'warning';
      melding.value = `Bewaard als branch ${branch}, maar het laden van de nieuwe variant lukte niet (${e?.message ?? e}). Herlaad de pagina.`;
    }
  } catch (e) {
    meldingSoort.value = 'critical';
    melding.value = `Bewaren mislukt: ${e?.message ?? e}`;
  } finally {
    bezig.value = false;
  }
}
</script>

<style scoped>
.wv {
  display: flex; flex-wrap: wrap; align-items: center; gap: var(--primitives-space-12);
  padding: var(--primitives-space-8) var(--primitives-space-24);
  border-bottom: 1px solid var(--semantics-dividers-color);
  background: var(--semantics-surfaces-tinted-background-color);
}
.wv-variant { background: var(--primitives-color-lintblauw-50); }
.wv-links, .wv-rechts, .wv-knoppen { display: flex; align-items: center; gap: var(--primitives-space-8); flex-wrap: wrap; }
.wv-rechts { margin-left: auto; }
.wv-label { font-weight: 600; }
.wv-status { font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.wv-bewerkt .wv-status { color: var(--semantics-content-warning-color); font-weight: 600; }
.wv-breed { flex-basis: 100%; }
.wv-opslaan { display: flex; flex-direction: column; gap: var(--primitives-space-8); padding: var(--primitives-space-8) 0; }
.wv-uitleg { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.wv-velden { display: flex; gap: var(--primitives-space-12); flex-wrap: wrap; }
.wv-velden nldd-form-field { min-width: 280px; }
</style>
