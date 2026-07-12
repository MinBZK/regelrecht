/**
 * useDocumentsSheet - shared open/close state for the traject-documents
 * browser sheet.
 *
 * The sheet is triggered from the TrajectMenu ("Documenten…") but rendered
 * once per app (EditorApp / LibraryApp) by `TrajectDocuments.vue`. A
 * module-level singleton ref lets the menu (which may be mounted several
 * times for the responsive headers) drive the single rendered sheet without
 * prop-drilling or duplicate instances.
 */
import { ref } from 'vue';

const isOpen = ref(false);

export function useDocumentsSheet() {
  return {
    isOpen,
    open() {
      isOpen.value = true;
    },
    close() {
      isOpen.value = false;
    },
  };
}
