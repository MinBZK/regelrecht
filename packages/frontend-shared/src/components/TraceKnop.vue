<template>
  <!-- Een klein RegelRecht-icoon naast een uitkomst die uit een engine-run
       komt. Het opent een sheet met de trace van die run: welke artikelen,
       welke waarden, welke stappen. Een sheet en geen modal: de trace is
       secundaire inhoud naast de uitkomst, geen onderbreking. -->
  <nldd-icon-button
    variant="neutral-transparent"
    size="sm"
    popup-type="dialog"
    :expanded="open || undefined"
    :accessible-label="`Hoe dit is berekend: ${titel}`"
    @click="open = true"
  >
    <img slot="icon" :src="icoon" alt="" width="20" height="20" />
  </nldd-icon-button>
  <nldd-sheet
    ref="sheetEl"
    placement="right"
    width="760px"
    :accessible-label="`Hoe dit is berekend: ${titel}`"
    @close="open = false"
  >
    <nldd-page>
      <nldd-container padding="24" gap="16">
        <nldd-title :size="3">
          <span slot="overline">Trace van de engine</span>
          <span>{{ titel }}</span>
        </nldd-title>
        <nldd-rich-text v-if="toelichting"><p>{{ toelichting }}</p></nldd-rich-text>
        <ul v-if="wortel" class="rr-boom">
          <TraceNode :node="wortel" />
        </ul>
        <nldd-rich-text v-else><p>Deze run gaf geen trace terug.</p></nldd-rich-text>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>

<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import TraceNode from './TraceNode.vue';
import icoon from '../assets/regelrecht-icon.svg';

const props = defineProps({
  // De trace zoals de engine hem geeft: een PathNode, of een tracedocument
  // ({ trace_version, root }, RFC-039).
  trace: { type: Object, default: null },
  titel: { type: String, required: true },
  toelichting: { type: String, default: '' },
});

const open = ref(false);
const sheetEl = ref(null);
// Een tracedocument heeft een `root`; zonder die sleutel is het de boom zelf.
const wortel = computed(() => {
  const t = props.trace;
  if (!t) return null;
  return 'trace_version' in t ? (t.root ?? null) : t;
});

// Spiegel de open-toestand naar de imperatieve API van de sheet, zodat de
// animatie speelt. @close zet de toestand terug; de watch roept dan nog een
// keer hide(), wat op een gesloten sheet niets doet.
watch(open, async (o) => {
  if (!o) {
    sheetEl.value?.hide();
    return;
  }
  await nextTick();
  sheetEl.value?.show();
});
</script>

<style scoped>
/* De boom is een <ul> van TraceNode-items; alleen de standaard inspringing van
   de browser eraf. */
.rr-boom {
  margin: 0;
  padding: 0;
}
</style>
