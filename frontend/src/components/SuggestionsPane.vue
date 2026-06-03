<script setup>
/**
 * SuggestionsPane — lists AI-generated suggestions (Aanwijzingen voor de
 * regelgeving + machine_readable) for the selected article, with accept/reject
 * per item. Suggestions are W3C annotations produced by the pipeline; the
 * dotted-span highlighting in the Tekst pane is handled by AnnotatedText (it
 * already styles `creator: Agent/llm` as generated). This pane is the list view.
 *
 * Built from NDD (`nldd-*`) components only. Accept/reject are emitted to the
 * parent (EditorApp), which applies a text/YAML change or marks the suggestion
 * resolved via the annotations write path.
 */
import { computed } from 'vue';

const props = defineProps({
  // [{ note, spans }] for the active article (from useSuggestions).
  suggestionsForArticle: { type: Array, default: () => [] },
  // [{ note, reason }] orphaned/ambiguous suggestions.
  issues: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
  // 'idle' | 'running' | 'done'
  jobState: { type: String, default: 'idle' },
});

const emit = defineEmits(['accept', 'reject']);

// Extract the plain suggestion text from a note's body (one body or a list).
function bodyText(note) {
  const body = note?.body;
  if (!body) return '';
  const first = Array.isArray(body) ? body[0] : body;
  return first?.value ?? '';
}

// Creator name (Agent) for a subtle source label per item.
function creatorName(note) {
  const c = note?.creator;
  if (!c) return 'AI';
  return typeof c === 'string' ? c : (c.name ?? 'AI');
}

// `editing` motivation means a concrete replacement is offered → show "Toepassen".
function hasReplacement(note) {
  return note?.motivation === 'editing';
}

const items = computed(() =>
  props.suggestionsForArticle.map((s, i) => ({
    key: `${s.note?.target?.selector?.exact ?? 'sug'}-${i}`,
    note: s.note,
    text: bodyText(s.note),
    creator: creatorName(s.note),
    applicable: hasReplacement(s.note),
  })),
);
</script>

<template>
  <nldd-simple-section width="full">
    <!-- Status while suggestion jobs run in the background. -->
    <nldd-inline-dialog
      v-if="jobState === 'running'"
      data-testid="suggestions-running"
      text="Aanwijzingen worden gegenereerd…"
      supporting-text="Dit kan een paar minuten duren. De suggesties verschijnen hier zodra ze klaar zijn."
    ></nldd-inline-dialog>

    <nldd-inline-dialog
      v-else-if="loading"
      data-testid="suggestions-loading"
      text="Suggesties laden…"
    ></nldd-inline-dialog>

    <nldd-inline-dialog
      v-else-if="items.length === 0 && issues.length === 0"
      data-testid="suggestions-empty"
      text="Geen suggesties voor dit artikel"
      supporting-text="Sla de wet op om nieuwe suggesties te laten genereren."
    ></nldd-inline-dialog>

    <!-- The suggestions for the selected article. -->
    <nldd-list v-if="items.length" variant="box" data-testid="suggestions-list">
      <nldd-list-item
        v-for="item in items"
        :key="item.key"
        size="md"
        :data-testid="`suggestion-${item.key}`"
      >
        <nldd-cell width="full">
          <nldd-text-cell :text="item.text" :supporting-text="item.creator"></nldd-text-cell>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button-group>
            <nldd-button
              v-if="item.applicable"
              variant="primary"
              size="sm"
              text="Toepassen"
              :data-testid="`accept-${item.key}`"
              @click="emit('accept', item.note)"
            ></nldd-button>
            <nldd-button
              variant="subtle"
              size="sm"
              text="Afwijzen"
              :data-testid="`reject-${item.key}`"
              @click="emit('reject', item.note)"
            ></nldd-button>
          </nldd-button-group>
        </nldd-cell>
      </nldd-list-item>
    </nldd-list>

    <!-- Suggestions that couldn't be anchored, surfaced separately. -->
    <template v-if="issues.length">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title text="Niet verankerd" size="sm"></nldd-title>
      <nldd-list variant="box" data-testid="suggestion-issues">
        <nldd-list-item v-for="(issue, i) in issues" :key="`issue-${i}`" size="md">
          <nldd-text-cell
            :text="issue.note?.body?.[0]?.value ?? issue.note?.body?.value ?? 'Suggestie'"
            :supporting-text="issue.reason"
          ></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </nldd-simple-section>
</template>
