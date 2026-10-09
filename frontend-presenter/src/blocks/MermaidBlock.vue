<script setup>
import { onMounted, ref, watch } from 'vue';
import { toRgb } from '../lib/cssColor.js';

// A ```mermaid block. Mermaid is large (~1 MB), so it is imported only when a
// slide actually has a diagram, and initialised once.
//
// The theme reads the design system's tokens at runtime instead of copying hex
// values: the same colours as the deck, and a palette change follows by itself.

const props = defineProps({ source: { type: String, required: true } });
const host = ref(null);
const svg = ref('');
const error = ref(null);

let mermaidPromise = null;
let counter = 0;

/**
 * A palette token as a plain rgb() string, the only kind mermaid can read.
 * Two steps, because the token is neither:
 * - it is a `light-dark(…)` expression; a probe element with the deck's
 *   `color-scheme: light` lets the browser resolve it to the light side, the
 *   side the deck always uses;
 * - the resolved colour comes back in oklch, the space the design system
 *   defines its primitives in, which mermaid rejects ("Unsupported color
 *   format"); toRgb converts it to sRGB.
 * When either step yields nothing, the hex fallback is used.
 */
function token(name, fallback) {
  const probe = document.createElement('span');
  probe.style.cssText = `color-scheme: light; color: var(${name}, ${fallback}); display: none`;
  document.body.appendChild(probe);
  const v = getComputedStyle(probe).color;
  probe.remove();
  return (v && toRgb(v)) || fallback;
}

function loadMermaid() {
  mermaidPromise ??= import('mermaid').then(({ default: mermaid }) => {
    const ink = token('--primitives-color-lintblauw-750', '#154273');
    const line = token('--primitives-color-lintblauw-500', '#4f6f9a');
    const fill = token('--primitives-color-coolgray-0', '#ffffff');
    const soft = token('--primitives-color-lintblauw-50', '#eef3f9');
    const accent = token('--primitives-color-donkergeel-200', '#ffb612');
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: 'base',
      fontFamily: "'RijksSans', system-ui, sans-serif",
      themeVariables: {
        fontFamily: "'RijksSans', system-ui, sans-serif",
        fontSize: '18px',
        primaryColor: soft,
        primaryBorderColor: ink,
        primaryTextColor: ink,
        secondaryColor: fill,
        tertiaryColor: fill,
        lineColor: line,
        textColor: ink,
        clusterBkg: fill,
        clusterBorder: line,
        edgeLabelBackground: fill,
        noteBkgColor: accent,
      },
      flowchart: { htmlLabels: false, curve: 'basis', padding: 16 },
    });
    return mermaid;
  });
  return mermaidPromise;
}

async function render() {
  error.value = null;
  try {
    const mermaid = await loadMermaid();
    const { svg: out } = await mermaid.render(`mmd-${Date.now()}-${counter++}`, props.source);
    svg.value = out;
  } catch (e) {
    svg.value = '';
    error.value = e?.message ?? String(e);
  }
}

onMounted(render);
watch(() => props.source, render);
</script>

<template>
  <figure ref="host" class="card mermaid">
    <pre v-if="error" class="mermaid-error">{{ error }}</pre>
    <!-- Mermaid's own output under securityLevel 'strict' (labels escaped). -->
    <div v-else class="mermaid-svg" v-html="svg"></div>
  </figure>
</template>
