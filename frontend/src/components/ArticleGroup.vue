<script setup>
import { computed } from 'vue';
import ArticleText from './ArticleText.vue';
import MachineReadable from './MachineReadable.vue';
import YamlView from './YamlView.vue';

/**
 * Read-only detail renderer for one *article* that may be stored as several
 * `articles[]` segments (harvester leaf-splitting, e.g. `1.e.1°`, `2.4.1.a.1°`).
 *
 * The article reads as one continuous whole:
 *  - Tekst: the segments' text stacked without separators — `1.a`, `1.e`, … just
 *    flow on as the article's content.
 *  - Machine / YAML: shown per segment that actually carries `machine_readable`,
 *    each with its segment number as provenance so it's clear which lid/onderdeel
 *    the logic belongs to. The label is only shown when the article is genuinely
 *    split (more than one segment), so single-entry laws look exactly as before.
 *
 * Reuses the existing per-segment components (`ArticleText`, `MachineReadable`,
 * `YamlView`) rather than re-implementing their rendering.
 */
const props = defineProps({
  segments: { type: Array, default: () => [] },
  // 'tekst' | 'machine' | 'yaml'
  view: { type: String, default: 'tekst' },
  // Passed through to MachineReadable for traject-scoped law-name resolution.
  trajectRef: { type: String, default: null },
  centered: { type: Boolean, default: false },
});

// `open-action` carries the owning segment alongside the action, so the
// ActionSheet resolves variables/definitions against the right segment's
// machine_readable (a split article's actions can live on any sub-segment).
const emit = defineEmits(['open-action']);

// Only segments that carry machine_readable matter for the Machine/YAML tabs;
// rendering an empty "geen gegevens" block per leaf segment would be noise.
const mrSegments = computed(() =>
  props.segments.filter((s) => s.machine_readable),
);

// Show the per-segment provenance label only for genuinely split articles.
const showProvenance = computed(() => props.segments.length > 1);
</script>

<template>
  <!-- Tekst: one continuous article, segments flow on without separators. -->
  <template v-if="view === 'tekst'">
    <template v-for="(segment, i) in segments" :key="segment.number">
      <nldd-spacer v-if="i > 0" size="16"></nldd-spacer>
      <ArticleText :article="segment" :centered="centered || undefined" />
    </template>
  </template>

  <!-- Machine: per segment that has machine_readable, with provenance. -->
  <template v-else-if="view === 'machine'">
    <nldd-inline-dialog
      v-if="mrSegments.length === 0"
      text="Geen machine-leesbare gegevens voor dit artikel"
    ></nldd-inline-dialog>
    <template v-else>
      <template v-for="(segment, i) in mrSegments" :key="segment.number">
        <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
        <nldd-title v-if="showProvenance" size="6"><h4>Artikel {{ segment.number }}</h4></nldd-title>
        <nldd-spacer v-if="showProvenance" size="8"></nldd-spacer>
        <MachineReadable
          :article="segment"
          :traject-ref="trajectRef"
          @open-action="emit('open-action', { action: $event, article: segment })"
        />
      </template>
    </template>
  </template>

  <!-- YAML: per segment that has machine_readable, with provenance. -->
  <template v-else-if="view === 'yaml'">
    <nldd-inline-dialog
      v-if="mrSegments.length === 0"
      text="Geen machine-leesbare gegevens voor dit artikel"
    ></nldd-inline-dialog>
    <template v-else>
      <template v-for="(segment, i) in mrSegments" :key="segment.number">
        <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
        <nldd-title v-if="showProvenance" size="6"><h4>Artikel {{ segment.number }}</h4></nldd-title>
        <nldd-spacer v-if="showProvenance" size="8"></nldd-spacer>
        <YamlView :article="segment" />
      </template>
    </template>
  </template>
</template>
