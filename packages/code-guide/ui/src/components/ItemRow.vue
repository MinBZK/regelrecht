<script setup>
import { computed } from 'vue';

// A type or free function of a module as a row that opens its page.
const props = defineProps({
  g: { type: Object, required: true },
  module: { type: Object, required: true },
  item: { type: Object, required: true },
  kind: { type: String, required: true },
});

const name = computed(() => props.item.key ?? props.item.name);
const href = computed(() => props.g.hrefFor({ module: props.module.path, item: name.value }));
const callers = computed(() => props.item.callers);
</script>

<template>
  <nldd-list-item :href="href">
    <nldd-text-cell :text="name" :supporting-text="item.doc || 'No documentation'" />
    <nldd-cell v-if="!item.doc"><nldd-tag size="sm" color="warning" text="undocumented" /></nldd-cell>
    <nldd-cell v-if="item.vis !== 'pub'"><nldd-tag size="sm" :text="item.vis" /></nldd-cell>
    <nldd-cell v-if="item.methods"><nldd-tag size="sm" :text="`${item.methods} methods`" /></nldd-cell>
    <nldd-cell>
      <nldd-tag
        size="sm"
        :color="callers ? 'neutral' : 'warning'"
        :text="callers ? `called from ${callers} ${callers === 1 ? 'function' : 'functions'}` : 'not called'"
      />
    </nldd-cell>
    <nldd-cell v-if="item.stale"><nldd-tag size="sm" color="warning" text="changed since the index" /></nldd-cell>
    <nldd-cell><nldd-tag size="sm" :text="kind" /></nldd-cell>
  </nldd-list-item>
</template>
