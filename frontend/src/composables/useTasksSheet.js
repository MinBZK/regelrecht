/**
 * useTasksSheet - gedeelde open/close-state voor de taken-sheet.
 * Zelfde singleton-patroon als useDocumentsSheet.
 */
import { ref } from 'vue';

const isOpen = ref(false);

export function useTasksSheet() {
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
