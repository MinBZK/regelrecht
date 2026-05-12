<script setup>
import { computed, ref, watch } from 'vue';
import { useEditor, EditorContent } from '@tiptap/vue-3';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from 'tiptap-markdown';

const props = defineProps({
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
  /** Error from the most recent save attempt (Error instance or null) */
  saveError: { type: Object, default: null },
  /**
   * Markdown source, v-model-bound to the parent editor state.
   *
   * WARNING: tiptap-markdown is configured with `html: false`, so raw HTML
   * tags in the incoming markdown (e.g. `<em>`, `<sup>`, `<br>` from a
   * harvested corpus entry) are silently dropped on load. Editing such an
   * article and saving will strip the HTML from the persisted YAML even if
   * the user typed nothing — the diff at the first save will be larger than
   * the "numbered-prose normalization" framing in the PR description
   * suggests. The engine ignores `text` so this is functionally lossless,
   * but the rendered output in downstream readers will change.
   */
  modelValue: { type: String, default: '' },
});

const emit = defineEmits(['update:modelValue']);

const editor = useEditor({
  content: props.modelValue,
  editable: props.editable,
  extensions: [
    StarterKit,
    // Strict commonmark — HTML embedded in source markdown is dropped rather
    // than partially mapping into the prosemirror schema.
    //
    // Side effect: editing an article whose stored `text` already contains
    // raw HTML (e.g. <em>, <sup>, <br> from a harvested corpus entry) will
    // strip that HTML on first save. The engine ignores `text`, so this is
    // functionally lossless, but it produces a larger first-save diff than
    // the PR description's "numbered-prose normalization" implies. If the
    // corpus grows HTML-heavy `text` fields later, revisit by either
    // tolerating html: true with explicit schema mappings, or pre-stripping
    // and surfacing a warning before the user starts typing.
    Markdown.configure({
      html: false,
      tightLists: true,
      bulletListMarker: '-',
      transformPastedText: true,
    }),
  ],
  onUpdate: ({ editor }) => {
    emit('update:modelValue', editor.storage.markdown.getMarkdown());
  },
});

// Reactive tick used to re-evaluate `editor.isActive(...)` in the template
// on every selection change. Without this the toolbar active-state never
// updates because the editor instance reference is stable.
const selectionTick = ref(0);
watch(editor, (inst) => {
  if (!inst) return;
  const bump = () => { selectionTick.value++; };
  // `selectionUpdate` alone misses Ctrl+B style commands that change marks
  // without moving the cursor. `transaction` covers those, but fires on
  // every keystroke too — gate it on a doc-or-selection change so the
  // toolbar re-render doesn't run on pure cursor motion within a paragraph.
  const onTransaction = ({ transaction: tr }) => {
    if (tr.docChanged || tr.selectionSet) bump();
  };
  inst.on('selectionUpdate', bump);
  inst.on('transaction', onTransaction);
  // Return a cleanup so Vue removes the listeners if the watcher re-runs
  // (HMR while the component stays mounted re-evaluates the callback with
  // the same editor instance, which would otherwise stack listeners).
  return () => {
    inst.off('selectionUpdate', bump);
    inst.off('transaction', onTransaction);
  };
}, { immediate: true });

// Re-seed content when the parent swaps articles or a save completes. Skip if
// the markdown already matches what the editor holds — calling setContent on
// every keystroke would reset the cursor.
//
// Track the previously-rendered article so we only reset the cursor on an
// actual article switch. ProseMirror otherwise keeps the previous cursor
// offset, which can land in the middle of the new article (or snap to the
// end) and confuses the user. Within a single article we leave the selection
// alone — e.g. a server-side save that round-trips an unchanged markdown
// string should not yank the cursor to position 0.
let lastArticleNumber = props.article ? String(props.article.number) : null;
watch(() => props.modelValue, (next) => {
  const inst = editor.value;
  if (!inst) return;
  const current = inst.storage.markdown.getMarkdown();
  if (current === next) return;
  inst.commands.setContent(next || '', { emitUpdate: false });
  const currentArticleNumber = props.article ? String(props.article.number) : null;
  if (currentArticleNumber !== lastArticleNumber) {
    inst.commands.setTextSelection(0);
    lastArticleNumber = currentArticleNumber;
  }
});

watch(() => props.editable, (next) => {
  editor.value?.setEditable(next);
});

// Compute the active-format map once per selection/transaction tick. The
// template binds against `activeFormats.bold` etc., which makes the
// reactive dependency on `selectionTick` explicit (no side-effect read +
// lint suppression). Add another mark name here when the toolbar grows a
// new format button.
const activeFormats = computed(() => {
  // Subscribe to selection/transaction changes.
  selectionTick.value;
  const inst = editor.value;
  if (!inst) return { bold: false, italic: false, bulletList: false, orderedList: false };
  return {
    bold: inst.isActive('bold'),
    italic: inst.isActive('italic'),
    bulletList: inst.isActive('bulletList'),
    orderedList: inst.isActive('orderedList'),
  };
});

function toggleBold() { editor.value?.chain().focus().toggleBold().run(); }
function toggleItalic() { editor.value?.chain().focus().toggleItalic().run(); }
function toggleBulletList() { editor.value?.chain().focus().toggleBulletList().run(); }
function toggleOrderedList() { editor.value?.chain().focus().toggleOrderedList().run(); }

// Expose the active-format state and toggle handlers so the parent can
// render the formatting buttons inside its own pane-header (next to the
// existing pane-view dropdown) rather than this component re-drawing its
// own toolbar with a duplicate label dropdown.
defineExpose({
  activeFormats,
  toggleBold,
  toggleItalic,
  toggleBulletList,
  toggleOrderedList,
});
</script>

<template>
  <div class="article-text-editor" data-testid="article-text-editor">
    <div class="article-text-editor__body-wrap">
      <template v-if="article && editable && saveError">
        <nldd-inline-dialog
          variant="alert"
          text="Opslaan mislukt"
          :supporting-text="saveError.message || String(saveError)"
          data-testid="save-text-error"
        ></nldd-inline-dialog>
        <nldd-spacer size="12"></nldd-spacer>
      </template>

      <div v-if="!article" class="article-text-editor__empty">
        <nldd-inline-dialog text="Geen artikel geselecteerd"></nldd-inline-dialog>
      </div>
      <editor-content v-else :editor="editor" class="article-text-editor__body" />
    </div>
  </div>
</template>

<style scoped>
.article-text-editor {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.article-text-editor__body-wrap {
  padding: 16px;
}

.article-text-editor__empty {
  padding: 32px 16px;
  text-align: center;
}

.article-text-editor__body {
  background: var(--semantics-surfaces-tinted-background-color, #F4F6F9);
  border: 1px solid var(--semantics-borders-default-color, #DDE0E4);
  border-radius: 12px;
  padding: 16px;
  min-height: 200px;
  line-height: 1.6;
  font-size: 14px;
}

.article-text-editor__body :deep(.ProseMirror) {
  outline: none;
  min-height: 160px;
}

.article-text-editor__body :deep(.ProseMirror p) {
  margin: 0 0 12px;
}

.article-text-editor__body :deep(.ProseMirror p:last-child) {
  margin-bottom: 0;
}

.article-text-editor__body :deep(.ProseMirror ul),
.article-text-editor__body :deep(.ProseMirror ol) {
  margin: 0 0 12px;
  padding-left: 24px;
}

.article-text-editor__body :deep(.ProseMirror li) {
  margin-bottom: 4px;
}
</style>
