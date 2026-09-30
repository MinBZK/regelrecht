<script setup>
/**
 * Workaround voor nldd-banner: via document.createElement raakt de
 * banner-definitie in een 'failed' staat (HTMLUnknownElement) en rendert
 * hij nooit; via de HTML-parser werkt hij wel. Dit component voegt de
 * banner daarom met innerHTML in. Vermoedelijke bug in het design system;
 * gemeld als aandachtspunt.
 */
import { computed } from 'vue';

const props = defineProps({
  variant: { type: String, default: 'neutral' },
  text: { type: String, default: '' },
  supportingText: { type: String, default: '' },
});

function esc(s) {
  return String(s)
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}

const html = computed(
  () =>
    `<nldd-banner variant="${esc(props.variant)}" text="${esc(props.text)}"` +
    (props.supportingText ? ` supporting-text="${esc(props.supportingText)}"` : '') +
    '></nldd-banner>',
);
</script>

<template>
  <div v-html="html"></div>
</template>
