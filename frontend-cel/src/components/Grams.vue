<script setup>
// Grams as a table, with the raw YAML below. Every gram has its own id, and
// refers with a name from the law to the gram it belongs to (a decision
// on_application, a payment to the decision). Every gram has two times:
// effective_at, when the fact holds in law (for an application from the
// counter the day of receipt), and recorded_at, when the cell recorded it.
const props = defineProps({
  // [{gram, yaml}], in the order in which they are shown.
  items: { type: Array, required: true },
  // The gram just recorded, to point it out in the table.
  highlighted: { type: Object, default: null },
  emptyText: { type: String, default: undefined },
});

// The id from the cell identifies the gram.
const key = (g) => g.id;
const references = (g) =>
  Object.entries(g.refers_to ?? {})
    .map(([name, id]) => `${name}: ${id}`)
    .join(', ') || undefined;
const isHighlighted = (g) => props.highlighted !== null && key(g) === key(props.highlighted);
</script>

<template>
  <nldd-table
    columns="190px 190px minmax(110px,1fr) minmax(150px,1fr) 130px minmax(180px,1fr) 110px"
    accessible-label="Vastgelegde grammen"
    empty-text="Nog niets vastgelegd"
    :empty-supporting-text="emptyText"
  >
    <nldd-table-row slot="header">
      <nldd-text-cell text="Op moment"></nldd-text-cell>
      <nldd-text-cell text="Vastgelegd op"></nldd-text-cell>
      <nldd-text-cell text="Kroniek"></nldd-text-cell>
      <nldd-text-cell text="Event"></nldd-text-cell>
      <nldd-text-cell text="Type"></nldd-text-cell>
      <nldd-text-cell text="Id en verwijzingen"></nldd-text-cell>
      <nldd-text-cell text="Herkomst"></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="(i, n) in items" :key="n + key(i.gram)" :selected="isHighlighted(i.gram) || undefined">
      <nldd-text-cell :text="i.gram.effective_at"></nldd-text-cell>
      <nldd-text-cell :text="i.gram.recorded_at || i.gram.effective_at"></nldd-text-cell>
      <nldd-text-cell :text="i.gram.chronicle"></nldd-text-cell>
      <nldd-text-cell :text="i.gram.name"></nldd-text-cell>
      <nldd-text-cell :text="[i.gram.type, i.gram.subtype, i.gram.stage].filter(Boolean).join(' / ')"></nldd-text-cell>
      <nldd-text-cell
        :text="i.gram.id"
        :supporting-text="references(i.gram)"
      ></nldd-text-cell>
      <nldd-text-cell :text="i.gram.provenance ?? 'vastgesteld'"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
  <template v-for="(i, n) in items" :key="'yaml-' + n + key(i.gram)">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="5"><h2>{{ i.gram.name }}, {{ i.gram.id }}</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-code-viewer language="yaml" wrap>{{ i.yaml }}</nldd-code-viewer>
  </template>
</template>
