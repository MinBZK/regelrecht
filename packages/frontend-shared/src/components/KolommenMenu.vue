<template>
  <!-- De knop draagt de stand ("2 van 3"), zodat dicht ook iets zegt. De
       varianten zelf staan in het menu, want hun titels zijn een halve regel
       lang en er kunnen er bij komen: een rij chips groeit mee met het aantal,
       een knop niet. -->
  <nldd-button
    expandable
    variant="neutral-tinted"
    size="sm"
    :text="knopTekst"
    :disabled="!varianten.length ? true : undefined"
  >
    <nldd-menu slot="popup" max-items="8" @select="opSelect">
      <nldd-menu-item
        v-for="v in varianten"
        :key="v.id"
        type="checkbox"
        :text="titel(v)"
        :value="v.id"
        :selected="gekozen.includes(v.id) ? true : undefined"
        :disabled="!gekozen.includes(v.id) && gekozen.length >= max ? true : undefined"
      ></nldd-menu-item>
      <span v-if="voet" slot="footer" class="km-voet">{{ voet }}</span>
    </nldd-menu>
  </nldd-button>
</template>

<!--
  Kolomkeuze naast de tabel die hij vult.

  De keuze stond in het linkerpaneel, terwijl alles wat hij bepaalt (de tabel en
  de twee grafieken) rechts staat. Links gaat over wat je wijzigt, en dit
  wijzigt niets: het kiest wat er naast huidig recht wordt doorgerekend.

  Waarom een menu en geen rij chips: varianten zijn niet met een vast aantal
  gegeven. Naast die uit het dossier maakt "Bewaar als variant" er zelf een, met
  een titel die de gebruiker kiest. Chips zouden de kop laten groeien met elke
  variant erbij, en hun labels waren al afgeknipt tot "nk-2: één definitie
  van…". In het menu past de hele titel en blijft de knop even breed.
-->

<script setup>
import { computed } from 'vue';

const props = defineProps({
  /** Alle beschikbare varianten: [{ id, title }]. */
  varianten: { type: Array, required: true },
  /** De gekozen variant-ids, in keuzevolgorde. */
  gekozen: { type: Array, required: true },
  /** Hoeveel varianten er naast huidig recht passen. */
  max: { type: Number, default: 3 },
  /** Hoe een variant zijn naam schrijft; per casus net anders. */
  titel: { type: Function, required: true },
  /**
   * Korte regel onder het menu, voor wat je niet uit de lijst afleidt. Houd hem
   * kort: hij bepaalt mede de hoogte van het menu, en een alinea leest hier
   * niemand. Leeg laten mag.
   */
  voet: { type: String, default: '' },
});
const emit = defineEmits(['update:gekozen']);

/**
 * Dicht moet de knop zeggen wat er staat. "Kolommen" alleen zou een knop zijn
 * waarvan je de stand niet ziet zonder hem te openen, en dat is precies wat het
 * paneel hiervoor wel kon.
 */
const knopTekst = computed(() => {
  if (!props.varianten.length) return 'Kolommen: geen varianten';
  if (!props.gekozen.length) return `Kolommen: alleen huidig recht`;
  return `Kolommen: ${props.gekozen.length} van ${props.max}`;
});

/**
 * Het menu-item vertelt zelf welk item is aangeklikt; de selectiestand komt van
 * ons, zodat het maximum ook klopt als er snel achter elkaar geklikt wordt.
 */
function opSelect(event) {
  const item = event.target.closest('nldd-menu-item');
  if (!item) return;
  const id = String(item.value);
  const aan = props.gekozen.includes(id);
  if (!aan && props.gekozen.length >= props.max) return;
  emit('update:gekozen', aan ? props.gekozen.filter((x) => x !== id) : [...props.gekozen, id]);
}
</script>

<style scoped>
/* De voetregel is de uitleg die eerst onder het paneel stond. Het menu laat
   zijn footer onopgemaakt, dus de marge en de toon komen van hier. */
.km-voet {
  display: block;
  padding: var(--primitives-space-4) var(--primitives-space-12) var(--primitives-space-8);
  font-size: 0.8em;
  line-height: 1.3;
  color: var(--semantics-content-secondary-color);
}
</style>
