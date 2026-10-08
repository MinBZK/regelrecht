<script setup>
import { computed } from 'vue';

const props = defineProps({ g: { type: Object, required: true } });

const title = computed(() => {
  const g = props.g;
  if (g.sourceTitle) return g.sourceTitle;
  if (g.method) return g.method.key;
  if (g.route.item) return g.route.item;
  if (g.module) return g.module.path || 'crate root';
  return 'Source';
});
const changedSince = computed(() => {
  const path = props.g.source?.path;
  return !!path && (props.g.status?.counts?.staleFiles ?? []).includes(path);
});
const subtitle = computed(() => {
  const s = props.g.source;
  if (!s) return props.g.range?.path ?? '';
  return `${s.path}, lines ${s.from} to ${s.to} of ${s.total}`;
});
// Re-created whenever the shown range changes: the viewer reads its content
// when it is attached, so a new range needs a new element.
const viewerKey = computed(() => (props.g.source ? `${props.g.source.path}:${props.g.source.from}:${props.g.source.to}` : 'none'));
</script>

<template>
  <nldd-page sticky-header accessible-label="Source code">
    <nldd-container slot="header" padding="16">
      <nldd-top-title-bar :text="title" :supporting-text="subtitle" heading-level="2">
        <!-- A link, like everything else: the address records whether the source is widened. -->
        <nldd-button
          slot="toolbar"
          :href="g.hrefHere({ wide: !g.route.wide })"
          :text="g.route.wide ? 'Back to details' : 'Widen'"
        />
      </nldd-top-title-bar>
    </nldd-container>

    <nldd-simple-section v-if="g.sourceError">
      <nldd-banner variant="critical" text="Could not read the source" :supporting-text="g.sourceError" />
    </nldd-simple-section>

    <nldd-simple-section v-else-if="!g.range">
      <nldd-text>Select a module, type or function to read its source, doc comment included.</nldd-text>
    </nldd-simple-section>

    <nldd-simple-section v-else-if="g.source">
      <!-- The file is read as it is now; what the guide says about it is as indexed. -->
      <template v-if="changedSince">
        <nldd-banner
          variant="warning"
          text="This file changed after indexing"
          supporting-text="The lines are found again by name in the current file; the calls the guide shows for it are still those from the index."
        />
        <nldd-spacer size="8" />
      </template>
      <nldd-button
        v-if="g.canShowMoreAbove || g.canShowMoreBelow"
        text="Show more lines around it"
        :disabled="g.sourceLoading"
        @click="g.showMoreContext()"
      />
      <nldd-spacer v-if="g.canShowMoreAbove || g.canShowMoreBelow" size="8" />
      <nldd-code-viewer :key="viewerKey" language="rust">{{ g.source.text }}</nldd-code-viewer>
    </nldd-simple-section>

    <nldd-simple-section v-else>
      <nldd-text>Reading the source…</nldd-text>
    </nldd-simple-section>
  </nldd-page>
</template>
