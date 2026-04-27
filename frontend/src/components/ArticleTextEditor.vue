<script setup>
import { ref, watch, onBeforeUnmount } from 'vue';
import { useEditor, EditorContent } from '@tiptap/vue-3';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from 'tiptap-markdown';

const props = defineProps({
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
  /** Error from the most recent save attempt (Error instance or null) */
  saveError: { type: Object, default: null },
  /** Markdown source, v-model-bound to the parent editor state */
  modelValue: { type: String, default: '' },
});

const emit = defineEmits(['update:modelValue']);

const editor = useEditor({
  content: props.modelValue,
  editable: props.editable,
  extensions: [
    StarterKit,
    Markdown.configure({
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
  // Listeners are freed when `editor.destroy()` runs in onBeforeUnmount; we
  // don't .off() explicitly because the editor instance is stable across the
  // component's lifetime.
  inst.on('selectionUpdate', bump);
  inst.on('transaction', bump);
}, { immediate: true });

// Re-seed content when the parent swaps articles or a save completes. Skip if
// the markdown already matches what the editor holds — calling setContent on
// every keystroke would reset the cursor.
watch(() => props.modelValue, (next) => {
  const inst = editor.value;
  if (!inst) return;
  const current = inst.storage.markdown.getMarkdown();
  if (current === next) return;
  inst.commands.setContent(next || '', { emitUpdate: false });
});

watch(() => props.editable, (next) => {
  editor.value?.setEditable(next);
});

onBeforeUnmount(() => {
  editor.value?.destroy();
});

function isActive(name, attrs) {
  // eslint-disable-next-line no-unused-expressions
  selectionTick.value;
  return editor.value?.isActive(name, attrs) ?? false;
}

function toggleBold() { editor.value?.chain().focus().toggleBold().run(); }
function toggleItalic() { editor.value?.chain().focus().toggleItalic().run(); }
function toggleBulletList() { editor.value?.chain().focus().toggleBulletList().run(); }
function toggleOrderedList() { editor.value?.chain().focus().toggleOrderedList().run(); }
</script>

<template>
  <div class="article-text-editor" data-testid="article-text-editor">
    <nldd-toolbar v-if="article" size="md" class="article-text-editor__toolbar">
      <nldd-toolbar-item slot="start">
        <!-- Single-option panel-label dropdown; future revisions will let users switch the pane to other views (e.g. structured outline). Disabled today because there's only one option. -->
        <nldd-dropdown size="md">
          <select disabled aria-label="Paneel" data-testid="article-text-panel-label">
            <option value="tekst">Tekst</option>
          </select>
        </nldd-dropdown>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="center">
        <div class="fmt-group">
          <span class="fmt-btn" :class="{ 'is-active': isActive('bold') }">
            <nldd-icon-button
              icon="bold"
              size="md"
              accessible-label="Vet"
              data-testid="fmt-bold"
              :disabled="!editable || undefined"
              @click="toggleBold"
            ></nldd-icon-button>
          </span>
          <span class="fmt-btn" :class="{ 'is-active': isActive('italic') }">
            <nldd-icon-button
              icon="italic"
              size="md"
              accessible-label="Schuin"
              data-testid="fmt-italic"
              :disabled="!editable || undefined"
              @click="toggleItalic"
            ></nldd-icon-button>
          </span>
          <span class="fmt-divider" role="separator" aria-orientation="vertical"></span>
          <span class="fmt-btn" :class="{ 'is-active': isActive('bulletList') }">
            <nldd-icon-button
              icon="bullet-list"
              size="md"
              accessible-label="Opsomming"
              data-testid="fmt-bullet-list"
              :disabled="!editable || undefined"
              @click="toggleBulletList"
            ></nldd-icon-button>
          </span>
          <span class="fmt-btn" :class="{ 'is-active': isActive('orderedList') }">
            <nldd-icon-button
              icon="numbered-list"
              size="md"
              accessible-label="Genummerde lijst"
              data-testid="fmt-ordered-list"
              :disabled="!editable || undefined"
              @click="toggleOrderedList"
            ></nldd-icon-button>
          </span>
        </div>
      </nldd-toolbar-item>
    </nldd-toolbar>

    <div class="article-text-editor__body-wrap">
      <template v-if="editable && saveError">
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

.article-text-editor__toolbar {
  /* Pin at the top of the pane body; the scrolling area is the editor body
   * below, not the toolbar. */
  position: sticky;
  top: 0;
  z-index: 1;
}

.fmt-group {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.fmt-btn {
  display: inline-flex;
  border-radius: 8px;
  transition: background-color 120ms ease;
}

/* Toggled formatting buttons: the library's nldd-icon-button has no built-in
 * pressed state, so we paint it ourselves. A follow-up upstream PR to
 * MinBZK/storybook can add a real `pressed` attr; until then this scoped
 * wrapper keeps the indication local. */
.fmt-btn.is-active {
  background-color: var(--semantics-surfaces-accent-tinted-background-color, rgba(0, 123, 199, 0.14));
}

/* Vertical separator between the inline/mark toggles and the list toggles.
 * `nldd-button-bar-divider` only renders when nested inside `nldd-button-bar`;
 * here we're already inside `nldd-toolbar-item`, so we draw the line locally. */
.fmt-divider {
  display: inline-block;
  width: 1px;
  height: 20px;
  margin: 0 4px;
  background-color: var(--semantics-borders-default-color, #DDE0E4);
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
