<script setup>
/**
 * CorpusGraafView — de afhankelijkheidsgraaf van een héél corpus.
 *
 * Onderscheidt zich van `LawGraphView`: die tekent één wortelwet met haar
 * transitieve afhankelijkheden op veld-niveau (inputs, outputs, concepten).
 * Deze tekent het corpus op wet-niveau — wie roept wie aan — omdat de vraag
 * hier "hoe hangt het stelsel samen" is en niet "hoe rekent deze wet".
 *
 * De data komt uit `lib/corpusgraaf.js`; dit component rekent niets uit. Wat
 * er niet in de graaf zit zit er bewust niet in: een `misplaced` binding
 * levert géén rand op, want de engine ziet 'm niet (zie de lib).
 *
 * Layout: rij per `regulatory_layer`, hoogste regeling boven. Bewust
 * deterministisch (alfabetisch binnen de rij) in plaats van een
 * force-simulatie — bouwplan §5 eist gelijke uitvoer bij gelijke invoer, en
 * een graaf die bij elke render anders ligt is niet te vergelijken met de
 * vorige snapshot.
 */
import { computed } from 'vue';
import { VueFlow, MarkerType, Position } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { MiniMap } from '@vue-flow/minimap';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import '@vue-flow/controls/dist/style.css';
import '@vue-flow/minimap/dist/style.css';

const props = defineProps({
  /** Uitvoer van `bouwGraaf()`. */
  graaf: { type: Object, required: true },
  naamVoor: { type: Function, default: (id) => id },
});

const emit = defineEmits(['knoop']);

// Hoog naar laag. Een onbekende laag zakt naar onderen in plaats van de
// volgorde te breken.
const LAAGVOLGORDE = [
  'GRONDWET',
  'EU_VERORDENING',
  'WET',
  'AMVB',
  'MINISTERIELE_REGELING',
  'GEMEENTELIJKE_VERORDENING',
  'WATERSCHAPS_VERORDENING',
  'BELEIDSREGEL',
];

const KOLOMBREEDTE = 260;
const RIJHOOGTE = 150;

function laagIndex(laag) {
  const i = LAAGVOLGORDE.indexOf(laag);
  return i === -1 ? LAAGVOLGORDE.length : i;
}

const nodes = computed(() => {
  // Groepeer per laag, alfabetisch binnen de laag: dezelfde invoer geeft
  // dezelfde coördinaten.
  const perLaag = new Map();
  for (const k of props.graaf.knopen) {
    const i = laagIndex(k.laag);
    if (!perLaag.has(i)) perLaag.set(i, []);
    perLaag.get(i).push(k);
  }
  const uit = [];
  for (const [rij, knopen] of [...perLaag.entries()].sort(([a], [b]) => a - b)) {
    knopen.sort((a, b) => a.lawId.localeCompare(b.lawId, 'nl'));
    knopen.forEach((k, kolom) => {
      uit.push({
        id: k.lawId,
        type: 'default',
        position: { x: kolom * KOLOMBREEDTE - ((knopen.length - 1) * KOLOMBREEDTE) / 2, y: rij * RIJHOOGTE },
        sourcePosition: Position.Bottom,
        targetPosition: Position.Top,
        data: k,
        label: props.naamVoor(k.lawId),
        class: [
          'corpusknoop',
          !k.aanwezig ? 'corpusknoop--ontbreekt' : '',
          k.nietAangeroepen && k.aanwezig ? 'corpusknoop--losstaand' : '',
        ]
          .filter(Boolean)
          .join(' '),
      });
    });
  }
  return uit;
});

const edges = computed(() =>
  props.graaf.randen
    // Een zelf-verwijzing (intra-law) is informatie over één knoop, geen
    // samenhang tussen knopen; die zou als lus alleen ruis toevoegen.
    .filter((r) => r.van !== r.naar)
    .map((r, i) => ({
      id: `${r.soort}:${r.van}->${r.naar}:${r.label}:${i}`,
      source: r.van,
      target: r.naar,
      label: r.label || undefined,
      animated: r.integriteit !== 'clean',
      // Stippel voor `implements`: de IoC-richting is een andere relatie dan
      // een waarde-ophaling, en dat onderscheid komt uit g.js' legenda.
      style: {
        strokeDasharray: r.soort === 'implements' ? '6 4' : undefined,
        stroke: r.integriteit === 'clean' ? undefined : 'var(--corpusgraaf-fout, #b8261a)',
      },
      markerEnd: MarkerType.ArrowClosed,
      class: `corpusrand corpusrand--${r.integriteit}`,
    })),
);

const legenda = computed(() => {
  const t = props.graaf.telling;
  const losstaand = props.graaf.knopen.filter((k) => k.nietAangeroepen && k.aanwezig).length;
  const ontbreekt = props.graaf.knopen.filter((k) => !k.aanwezig).length;
  return [
    { label: 'bindingen in orde', n: t.clean, waarschuwing: false },
    { label: 'dangling', n: t.dangling, waarschuwing: t.dangling > 0 },
    { label: 'implements dangling', n: t['impl-dangling'], waarschuwing: t['impl-dangling'] > 0 },
    { label: 'source op de verkeerde plek', n: t.misplaced, waarschuwing: t.misplaced > 0 },
    { label: 'niet aangeroepen', n: losstaand, waarschuwing: false },
    { label: 'doelwet ontbreekt', n: ontbreekt, waarschuwing: ontbreekt > 0 },
  ];
});
</script>

<template>
  <div class="corpusgraaf">
    <VueFlow
      :nodes="nodes"
      :edges="edges"
      fit-view-on-init
      :min-zoom="0.1"
      :max-zoom="2"
      @node-click="(e) => emit('knoop', e.node.data)"
    >
      <Background pattern-color="#d5d5d5" :gap="20" />
      <Controls />
      <MiniMap pannable zoomable />
    </VueFlow>
  </div>

  <nldd-spacer size="12"></nldd-spacer>
  <nldd-list variant="simple">
    <nldd-list-item v-for="rij in legenda" :key="rij.label" data-test="legenda">
      <nldd-icon-cell v-if="rij.waarschuwing" slot="start" size="20">
        <nldd-icon name="warning"></nldd-icon>
      </nldd-icon-cell>
      <nldd-text-cell :text="rij.label"></nldd-text-cell>
      <nldd-text-cell :text="String(rij.n)" horizontal-alignment="right"></nldd-text-cell>
    </nldd-list-item>
  </nldd-list>
</template>

<style scoped>
/* Vue Flow heeft een expliciete hoogte nodig; het ontwerpsysteem levert geen
   container met een vaste hoogte voor een canvas. Zelfde noodzaak als in
   LawGraphView. */
.corpusgraaf {
  height: 520px;
  width: 100%;
  border: 1px solid var(--nldd-color-border, #d5d5d5);
  border-radius: 8px;
  overflow: hidden;
}
</style>

<style>
/* Ongescoped: Vue Flow rendert knopen buiten de scope van dit component. */
.corpusknoop--losstaand {
  opacity: 0.55;
}
.corpusknoop--ontbreekt {
  border-style: dashed;
  border-color: #b8261a;
}
</style>
