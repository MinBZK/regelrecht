<script setup>
import { ref, computed } from 'vue';

const props = defineProps({
  /** Execution result with outputs */
  result: { type: Object, default: null },
  /** PathNode trace tree from engine */
  trace: { type: Object, default: null },
  /** Pre-rendered box-drawing trace text */
  traceText: { type: String, default: null },
  /** Expected output values: { outputName: expectedValue } */
  expectations: { type: Object, default: () => ({}) },
  /** Error message if execution failed */
  error: { type: String, default: null },
  /** Whether execution is running */
  running: { type: Boolean, default: false },
});

const viewMode = ref('tree');

// --- Output result formatting (migrated from ScenarioResults) ---

function formatValue(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  return String(value);
}

function formatOutputValue(value, name) {
  const raw = formatValue(value);
  if (typeof value === 'number' && isLikelyEurocent(name, value)) {
    return `${raw} (${(value / 100).toFixed(2)} euro)`;
  }
  return raw;
}

function isLikelyEurocent(name, value) {
  return (
    Number.isInteger(value) &&
    (name.includes('hoogte') || name.includes('bedrag') || name.includes('premie'))
  );
}

function matchStatus(outputName, actualValue) {
  if (!(outputName in props.expectations)) return 'neutral';
  const expected = props.expectations[outputName];
  if (expected === null || expected === undefined) return 'neutral';

  const actual = normalizeForCompare(actualValue);
  const exp = normalizeForCompare(expected);
  return primitiveEqual(actual, exp) ? 'passed' : 'failed';
}

function primitiveEqual(a, b) {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === 'number') return Math.abs(a - b) < 1e-9;
  return false;
}

function normalizeForCompare(value) {
  if (value === 'true' || value === true) return true;
  if (value === 'false' || value === false) return false;
  if (value === 'null' || value === null) return null;
  if (typeof value === 'string' && /^-?\d+(\.\d+)?$/.test(value)) return Number(value);
  return value;
}

// --- Trace tree ---

const hasContent = computed(() =>
  props.result || props.trace || props.traceText || props.error || props.running,
);

// Node type display config
const NODE_TYPE_CONFIG = {
  Article: { label: 'Artikel', class: 'trace-type--article' },
  UriCall: { label: 'URI', class: 'trace-type--uri' },
  Resolve: { label: 'Resolve', class: 'trace-type--resolve' },
  Operation: { label: 'Operatie', class: 'trace-type--operation' },
  Action: { label: 'Actie', class: 'trace-type--action' },
  Requirement: { label: 'Eis', class: 'trace-type--requirement' },
  Cached: { label: 'Cache', class: 'trace-type--cached' },
  OpenTermResolution: { label: 'Open term', class: 'trace-type--openterm' },
};

function nodeConfig(nodeType) {
  return NODE_TYPE_CONFIG[nodeType] || { label: nodeType, class: 'trace-type--default' };
}

function nodeResultText(node) {
  if (node.result === undefined || node.result === null) return null;
  return formatValue(node.result);
}
</script>

<template>
  <div class="etv-container">
    <div class="etv-scroll">
      <!-- Empty state -->
      <div v-if="!hasContent" class="etv-empty">
        <div class="etv-empty-text">Klik op "Uitvoeren" om de wet uit te voeren en de trace te bekijken.</div>
      </div>

      <!-- Running state -->
      <div v-else-if="running" class="etv-running">Uitvoeren...</div>

      <!-- Error state -->
      <div v-else-if="error && !result" class="etv-error">
        <div class="etv-error-title">Fout bij uitvoering</div>
        <div class="etv-error-message">{{ error }}</div>
      </div>

      <template v-if="result && !running">
        <!-- Output summary -->
        <div class="etv-section etv-outputs">
          <div class="etv-section-title">Resultaat</div>
          <div
            v-for="(value, name) in result.outputs"
            :key="name"
            class="etv-output"
            :class="`etv-output--${matchStatus(name, value)}`"
          >
            <span class="etv-output-icon">
              <template v-if="matchStatus(name, value) === 'passed'">&#x2713;</template>
              <template v-else-if="matchStatus(name, value) === 'failed'">&#x2717;</template>
              <template v-else>&#x25CF;</template>
            </span>
            <span class="etv-output-name">{{ name }}:</span>
            <span class="etv-output-value">{{ formatOutputValue(value, name) }}</span>
            <span
              v-if="matchStatus(name, value) === 'passed'"
              class="etv-badge etv-badge--pass"
            >GESLAAGD</span>
            <span
              v-if="matchStatus(name, value) === 'failed'"
              class="etv-badge etv-badge--fail"
            >MISLUKT (verwacht: {{ formatValue(expectations[name]) }})</span>
          </div>
        </div>

        <!-- Trace section -->
        <div v-if="trace || traceText" class="etv-section">
          <div class="etv-trace-header">
            <div class="etv-section-title">Execution trace</div>
            <div class="etv-view-toggle">
              <button
                :class="{ active: viewMode === 'tree' }"
                @click="viewMode = 'tree'"
              >Tree</button>
              <button
                :class="{ active: viewMode === 'text' }"
                @click="viewMode = 'text'"
              >Text</button>
            </div>
          </div>

          <!-- Tree view -->
          <div v-if="viewMode === 'tree' && trace" class="etv-tree">
            <TraceNode :node="trace" :depth="0" />
          </div>

          <!-- Text view -->
          <pre v-if="viewMode === 'text' && traceText" class="etv-trace-text">{{ traceText }}</pre>
        </div>
      </template>

      <!-- Partial trace on error -->
      <div v-if="error && (trace || traceText) && !running" class="etv-section">
        <div class="etv-trace-header">
          <div class="etv-section-title">Partial trace (tot fout)</div>
          <div class="etv-view-toggle">
            <button
              :class="{ active: viewMode === 'tree' }"
              @click="viewMode = 'tree'"
            >Tree</button>
            <button
              :class="{ active: viewMode === 'text' }"
              @click="viewMode = 'text'"
            >Text</button>
          </div>
        </div>
        <div v-if="viewMode === 'tree' && trace" class="etv-tree">
          <TraceNode :node="trace" :depth="0" />
        </div>
        <pre v-if="viewMode === 'text' && traceText" class="etv-trace-text">{{ traceText }}</pre>
      </div>
    </div>
  </div>
</template>

<!-- Recursive TraceNode as a nested component -->
<script>
import { defineComponent, ref as vueRef } from 'vue';

const TraceNode = defineComponent({
  name: 'TraceNode',
  props: {
    node: { type: Object, required: true },
    depth: { type: Number, default: 0 },
  },
  setup(props) {
    const expanded = vueRef(props.depth < 2);

    const config = NODE_TYPE_CONFIG[props.node.node_type] || { label: props.node.node_type, class: 'trace-type--default' };

    function resultText() {
      const r = props.node.result;
      if (r === undefined || r === null) return null;
      if (typeof r === 'boolean') return r ? 'ja' : 'nee';
      return String(r);
    }

    return { expanded, config, resultText };
  },
  template: `
    <div class="tn-node" :class="config.class">
      <div class="tn-header" @click="expanded = !expanded">
        <span v-if="node.children && node.children.length" class="tn-toggle">
          {{ expanded ? '\\u25BE' : '\\u25B8' }}
        </span>
        <span v-else class="tn-toggle tn-toggle--leaf">&middot;</span>
        <span class="tn-type-badge" :class="config.class">{{ config.label }}</span>
        <span class="tn-name">{{ node.name }}</span>
        <span v-if="node.resolve_type" class="tn-resolve-badge">{{ node.resolve_type }}</span>
        <span v-if="resultText()" class="tn-result">= {{ resultText() }}</span>
        <span v-if="node.duration_us >= 100" class="tn-duration">{{ node.duration_us }}\u00B5s</span>
      </div>
      <div v-if="node.message" class="tn-message">{{ node.message }}</div>
      <div v-if="expanded && node.children && node.children.length" class="tn-children">
        <TraceNode
          v-for="(child, i) in node.children"
          :key="i"
          :node="child"
          :depth="depth + 1"
        />
      </div>
    </div>
  `,
});

const NODE_TYPE_CONFIG = {
  Article: { label: 'Artikel', class: 'trace-type--article' },
  UriCall: { label: 'URI', class: 'trace-type--uri' },
  Resolve: { label: 'Resolve', class: 'trace-type--resolve' },
  Operation: { label: 'Operatie', class: 'trace-type--operation' },
  Action: { label: 'Actie', class: 'trace-type--action' },
  Requirement: { label: 'Eis', class: 'trace-type--requirement' },
  Cached: { label: 'Cache', class: 'trace-type--cached' },
  OpenTermResolution: { label: 'Open term', class: 'trace-type--openterm' },
};

export default {
  components: { TraceNode },
};
</script>

<style scoped>
.etv-container {
  height: 100%;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.etv-scroll {
  height: 100%;
  overflow-y: auto;
}

.etv-empty {
  padding: 32px 16px;
  text-align: center;
}

.etv-empty-text {
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #666);
}

.etv-running {
  padding: 12px 16px;
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #666);
  font-style: italic;
}

.etv-error {
  padding: 12px 16px;
}

.etv-error-title {
  font-weight: 600;
  font-size: 13px;
  color: #c00;
  margin-bottom: 4px;
}

.etv-error-message {
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  color: #c00;
  word-break: break-word;
  white-space: pre-wrap;
}

/* Sections */
.etv-section {
  padding: 12px 16px;
  border-bottom: 1px solid var(--semantics-dividers-color, #E0E3E8);
}

.etv-section-title {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 8px;
  color: var(--semantics-text-color-primary, #1C2029);
}

/* Output summary */
.etv-output {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 4px 0;
  font-size: 13px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}

.etv-output-icon {
  flex-shrink: 0;
  width: 14px;
  text-align: center;
  font-weight: bold;
}

.etv-output--passed .etv-output-icon { color: #060; }
.etv-output--failed .etv-output-icon { color: #c00; }
.etv-output--neutral .etv-output-icon { color: #666; }

.etv-output-name {
  font-weight: 600;
  color: var(--semantics-text-color-primary, #1C2029);
}

.etv-output-value {
  color: var(--semantics-text-color-secondary, #555);
}

.etv-badge {
  margin-left: auto;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 3px;
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.etv-badge--pass { background: #efe; color: #060; }
.etv-badge--fail { background: #fee; color: #c00; }

/* Trace header */
.etv-trace-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.etv-view-toggle {
  display: flex;
  gap: 2px;
}

.etv-view-toggle button {
  padding: 2px 10px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  background: var(--semantics-surfaces-color-secondary, #F0F1F3);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  color: var(--semantics-text-color-secondary, #666);
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
}

.etv-view-toggle button:first-child { border-radius: 4px 0 0 4px; }
.etv-view-toggle button:last-child { border-radius: 0 4px 4px 0; }

.etv-view-toggle button.active {
  background: #154273;
  color: white;
  border-color: #154273;
}

/* Box-drawing text */
.etv-trace-text {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 11px;
  line-height: 1.5;
  padding: 8px;
  background: #1e1e2e;
  color: #cdd6f4;
  border-radius: 6px;
  overflow-x: auto;
  white-space: pre;
  margin: 0;
}

/* Tree view */
.etv-tree {
  font-size: 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
}
</style>

<!-- Unscoped styles for recursive TraceNode -->
<style>
.tn-node {
  margin-left: 0;
}

.tn-children {
  margin-left: 16px;
  border-left: 1px solid var(--semantics-dividers-color, #E0E3E8);
  padding-left: 4px;
}

.tn-header {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 0;
  cursor: pointer;
  white-space: nowrap;
}

.tn-header:hover {
  background: var(--semantics-surfaces-color-secondary, #F8F9FA);
}

.tn-toggle {
  flex-shrink: 0;
  width: 14px;
  text-align: center;
  font-size: 10px;
  color: var(--semantics-text-color-secondary, #666);
}

.tn-toggle--leaf {
  color: var(--semantics-dividers-color, #ccc);
}

.tn-type-badge {
  flex-shrink: 0;
  font-size: 9px;
  font-weight: 700;
  padding: 0 4px;
  border-radius: 3px;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.trace-type--article .tn-type-badge,
.tn-type-badge.trace-type--article { background: #e8eaf6; color: #283593; }
.trace-type--uri .tn-type-badge,
.tn-type-badge.trace-type--uri { background: #e3f2fd; color: #1565c0; }
.trace-type--resolve .tn-type-badge,
.tn-type-badge.trace-type--resolve { background: #e8f5e9; color: #2e7d32; }
.trace-type--operation .tn-type-badge,
.tn-type-badge.trace-type--operation { background: #fff3e0; color: #e65100; }
.trace-type--action .tn-type-badge,
.tn-type-badge.trace-type--action { background: #fce4ec; color: #c62828; }
.trace-type--requirement .tn-type-badge,
.tn-type-badge.trace-type--requirement { background: #f3e5f5; color: #6a1b9a; }
.trace-type--cached .tn-type-badge,
.tn-type-badge.trace-type--cached { background: #eceff1; color: #546e7a; }
.trace-type--openterm .tn-type-badge,
.tn-type-badge.trace-type--openterm { background: #e0f7fa; color: #00695c; }
.tn-type-badge.trace-type--default { background: #f5f5f5; color: #666; }

.tn-name {
  color: var(--semantics-text-color-primary, #1C2029);
  overflow: hidden;
  text-overflow: ellipsis;
}

.tn-resolve-badge {
  flex-shrink: 0;
  font-size: 9px;
  font-weight: 600;
  padding: 0 3px;
  border-radius: 2px;
  background: #f0f1f3;
  color: #888;
  text-transform: uppercase;
}

.tn-result {
  color: #2e7d32;
  font-weight: 600;
}

.tn-duration {
  color: var(--semantics-text-color-secondary, #999);
  font-size: 10px;
  margin-left: auto;
}

.tn-message {
  font-size: 10px;
  color: var(--semantics-text-color-secondary, #888);
  padding-left: 18px;
  font-style: italic;
}
</style>
