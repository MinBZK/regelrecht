/**
 * Standalone demo page for the 3D corpus graph: `/graph3d.html`.
 *
 * It exists so the renderer can be looked at and driven at real corpus sizes
 * before the data layer serves the real graph, and so a size change is one
 * click instead of an edit. The graph is synthetic (see generateCorpusGraph);
 * the moment the endpoint exists, the same component takes the packed payload
 * from it and this page keeps working as the size harness.
 */
import './nldd-components.js';
import '@nldd/design-system/styles';
import { createApp, h, ref } from 'vue';
import CorpusGraph3DView from './components/CorpusGraph3DView.vue';

const SIZES = [
  // The real payloads, served under /corpusgraaf by the benchmark harness (and
  // by any dev server configured for it). The synthetic sizes stay behind them
  // so the renderer can still be pushed past what the corpus is today.
  { label: 'Echt corpus (wetniveau)', src: '/corpusgraaf/corpus-wetniveau.rrgraph' },
  {
    label: 'Echt corpus (artikelniveau)',
    src: '/corpusgraaf/corpus-artikelniveau.rrgraph',
    lawLevelOnly: false,
  },
  { label: 'Synthetisch 4.138', nodes: 4138, edges: 50000 },
  { label: '25.000 wetten', nodes: 25000, edges: 250000 },
  { label: '100.000 wetten', nodes: 100000, edges: 500000 },
  { label: '100.000 / 1M kanten', nodes: 100000, edges: 1000000 },
];

const app = createApp({
  setup() {
    const size = ref(0);
    const selected = ref(null);
    const key = ref(0);

    function pick(index) {
      size.value = index;
      key.value += 1;
    }

    return () =>
      h('nldd-page', {}, [
        h('nldd-top-title-bar', { slot: 'header', text: 'Corpusgraaf (3D)' }),
        h('nldd-simple-section', {}, [
          h(
            'nldd-button-bar',
            {},
            SIZES.map((s, i) =>
              h('nldd-button', {
                text: s.label,
                size: 'sm',
                variant: i === size.value ? 'primary' : 'secondary',
                onClick: () => pick(i),
              }),
            ),
          ),
          h(CorpusGraph3DView, {
            key: key.value,
            src: SIZES[size.value].src ?? null,
            lawLevelOnly: SIZES[size.value].lawLevelOnly !== false,
            nodes: SIZES[size.value].nodes ?? 4138,
            edges: SIZES[size.value].edges ?? 50000,
            onSelect: (node) => {
              selected.value = node;
            },
          }),
          h('nldd-rich-text', {}, [
            h(
              'p',
              {},
              selected.value
                ? selected.value.enriched
                  ? `Geselecteerd: ${selected.value.label} (graad ${selected.value.degree})`
                  : `${selected.value.label}: alleen geoogst, hier valt nog niets uit te klappen`
                : 'Klik een knoop aan voor selectie, dubbelklik om erheen te vliegen, dubbelklik op leegte voor het volledige overzicht.',
            ),
          ]),
        ]),
      ]);
  },
});

app.config.compilerOptions = { isCustomElement: (tag) => tag.startsWith('nldd-') };
app.mount('#app');
