<script setup>
import { computed } from 'vue';

// The backlog, read off the corpus: markings grouped by the change that would
// resolve them. One marking is an observation; the same change wanted by four
// articles across two laws is a pattern, and the pattern is what decides
// whether it is worth building.
//
// A sibling of the list rather than a separate view, because it is an
// aggregation over exactly the rows below it. Clicking a cluster filters that
// list, which is how a reader gets from "wanted in four places" to the four
// articles themselves.
const props = defineProps({
  clusters: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
});

defineEmits(['select']);

// A cluster of one is the interesting case, not the boring one. `resolved_by`
// is free text, so two agents asking for the same change in different words
// land in two clusters of one, and a reader has to be told that rather than
// left to infer it from a long tail.
const singletons = computed(() => props.clusters.filter((c) => c.markings === 1).length);

// A change only one provider ever asks for, while another ran over the same
// corpus without complaint, points at the enricher rather than at the format.
// The two are indistinguishable from the marking alone, so the signal has to
// be visible here.
function providerHint(cluster) {
  const providers = cluster.providers || [];
  if (providers.length > 1) return null;
  return providers[0] ? `alleen ${providers[0]}` : null;
}

function reachText(cluster) {
  const laws = cluster.laws === 1 ? '1 wet' : `${cluster.laws} wetten`;
  const articles = cluster.articles === 1 ? '1 artikel' : `${cluster.articles} artikelen`;
  return `${articles} in ${laws}`;
}
</script>

<template>
  <nldd-simple-section v-if="!loading && clusters.length">
    <nldd-title size="6"><h2>Wat het formaat mist</h2></nldd-title>
    <nldd-spacer size="4" />
    <nldd-rich-text>
      <p>
        Markeringen gegroepeerd op de wijziging die ze oplost. Meer wetten
        betekent een breder gat: iets wat in één wet speelt kan die wet zijn,
        iets wat in drie wetten speelt is het formaat.
      </p>
    </nldd-rich-text>
    <nldd-spacer size="8" />

    <nldd-list variant="simple">
      <nldd-list-item
        v-for="(cluster, i) in clusters"
        :key="`${cluster.resolution}-${cluster.resolved_by}-${i}`"
        button
        @click="$emit('select', cluster)"
      >
        <nldd-text-cell
          :overline="cluster.resolution === 'operation' ? 'bewerking' : 'formaat'"
          :text="cluster.resolved_by || 'niet benoemd'"
          :supporting-text="reachText(cluster)"
        />
        <nldd-spacer-cell />
        <nldd-text-cell
          v-if="providerHint(cluster)"
          :text="providerHint(cluster)"
          supporting-text="mogelijk de enricher"
          width="fit-content"
          min-width="120px"
          color="secondary"
        />
        <nldd-text-cell
          :text="String(cluster.markings)"
          supporting-text="markeringen"
          width="fit-content"
          min-width="90px"
          horizontal-alignment="right"
        />
      </nldd-list-item>
    </nldd-list>

    <template v-if="singletons">
      <nldd-spacer size="8" />
      <nldd-inline-dialog
        variant="alert"
        :text="`${singletons} ${singletons === 1 ? 'cluster telt' : 'clusters tellen'} één markering`"
        supporting-text="Groeperen gebeurt op de letterlijke tekst van 'wat het oplost', dus twee keer dezelfde wens in andere woorden staat hier twee keer. Die zijn het nakijken waard."
      />
    </template>
    <nldd-spacer size="16" />
  </nldd-simple-section>
</template>
