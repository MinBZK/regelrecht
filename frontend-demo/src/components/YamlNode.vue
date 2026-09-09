<script setup>
import { computed, ref, watch } from 'vue';

// One node of a parsed YAML document rendered as a collapsible tree. Mappings
// and sequences fold; scalars show typed. A `source.regulation: <law>` value
// becomes a link that opens that law in a new law tab.

const props = defineProps({
  name: { type: [String, Number], default: null },
  value: { default: null },
  path: { type: String, default: '' },
  depth: { type: Number, default: 0 },
  /** Dotted paths that start expanded (with their ancestors); '*' matches any segment. */
  expanded: { type: Object, default: () => ({ paths: [], version: 0, all: null }) },
  lawIds: { type: Object, default: () => new Set() },
  parentKey: { type: String, default: '' },
});
const emit = defineEmits(['open-law']);

const isMap = computed(() => props.value !== null && typeof props.value === 'object' && !Array.isArray(props.value));
const isList = computed(() => Array.isArray(props.value));
const isContainer = computed(() => isMap.value || isList.value);

function pathMatches(pattern, path) {
  const p = pattern.split('.');
  const q = path.split('.');
  if (p.length > q.length) return false;
  return p.every((seg, i) => seg === '*' || q[i] === '*' || seg === q[i]);
}

function initiallyOpen() {
  if (props.expanded.all === true) return true;
  if (props.expanded.all === false) return false;
  if (props.depth < 1) return true;
  // Articles and their execution blocks open by default; the long `text` and
  // the operations inside actions stay folded unless a configured path says so.
  if (/^articles(\.[^.]+)?$/.test(props.path)) return true;
  if (/^articles\.[^.]+\.machine_readable(\.execution)?$/.test(props.path)) return true;
  // A configured path opens itself and every node above it; what lies beside
  // or below it stays folded, so the prepared view lands on the right lines.
  return props.expanded.paths.some((pattern) => pathMatches(props.path, pattern));
}

const open = ref(initiallyOpen());
watch(() => props.expanded.version, () => {
  open.value = initiallyOpen();
});

const entries = computed(() => {
  if (isMap.value) return Object.entries(props.value);
  if (isList.value) return props.value.map((v, i) => [i, v]);
  return [];
});

/** Label used in the child path: items of a list get their name/output. */
function scalarHint(obj) {
  if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return undefined;
  // Only a scalar names a node; an execution block's `output` is a list.
  return [obj.output, obj.name, obj.number].find((h) => typeof h === 'string' || typeof h === 'number');
}

function childPath(key, child) {
  // Only a list item takes its name as label; a mapping key stays the key, so
  // `source: {output: x}` is addressed as `.source`, not `.x`.
  const hint = isList.value ? scalarHint(child) : undefined;
  // Paths are dot-separated, so a dot inside a label (article "2.34") would
  // split it into two segments and no default or configured path would match.
  const label = (hint !== undefined ? String(hint) : String(key)).replaceAll('.', '_');
  return props.path ? `${props.path}.${label}` : label;
}

function summary() {
  if (isList.value) return `${props.value.length} ${props.value.length === 1 ? 'item' : 'items'}`;
  const v = props.value;
  if (v.number !== undefined && v.text !== undefined) return `artikel ${v.number}`;
  const hint = scalarHint(v) ?? [v.operation, v.regulation].find((h) => typeof h === 'string');
  return hint !== undefined ? String(hint) : `${Object.keys(v).length} velden`;
}

const scalarClass = computed(() => {
  const v = props.value;
  if (typeof v === 'string' && v.startsWith('$')) return 'yaml-var';
  if (typeof v === 'string') return 'yaml-string';
  return 'yaml-scalar';
});

const isLawLink = computed(
  () => props.parentKey === 'source' && props.name === 'regulation' && typeof props.value === 'string',
);

// Long legal text (the `text` of an article) starts folded to its first
// sentence; the audience came for the machine-readable part.
const LONG = 220;
const isLong = computed(() => typeof props.value === 'string' && props.value.length > LONG);
const showAll = ref(false);

function scalarText() {
  const v = props.value;
  if (v === null || v === undefined) return 'null';
  if (typeof v === 'string') {
    if (isLong.value && !showAll.value) return `${v.slice(0, LONG).trimEnd()}…`;
    return v;
  }
  return String(v);
}
</script>

<template>
  <div class="yaml-node" :data-path="path">
    <template v-if="isContainer">
      <button type="button" class="yaml-toggle" :aria-expanded="open ? 'true' : 'false'" @click="open = !open">
        <span class="yaml-key">{{ name === null ? '' : typeof name === 'number' ? '-' : name }}</span><span v-if="name !== null && typeof name !== 'number'">:</span>
        <span v-if="!open" class="yaml-scalar"> {{ summary() }}</span>
      </button>
      <div v-if="open">
        <YamlNode
          v-for="[k, v] in entries"
          :key="k"
          :name="k"
          :value="v"
          :path="childPath(k, v)"
          :depth="depth + 1"
          :expanded="expanded"
          :law-ids="lawIds"
          :parent-key="isMap ? String(name ?? '') : parentKey"
          @open-law="emit('open-law', $event)"
        />
      </div>
    </template>
    <template v-else>
      <span class="yaml-key">{{ isList || typeof name === 'number' ? '-' : name }}</span><span v-if="!(isList || typeof name === 'number')">:</span>
      <button v-if="isLawLink && lawIds.has(value)" type="button" class="yaml-link" :title="`Open ${value}`" @click="emit('open-law', value)">
        {{ value }}
      </button>
      <span v-else :class="scalarClass" style="white-space: pre-wrap"> {{ scalarText() }}</span>
      <button v-if="isLong" type="button" class="yaml-link" @click="showAll = !showAll">{{ showAll ? 'minder' : 'meer' }}</button>
    </template>
  </div>
</template>
