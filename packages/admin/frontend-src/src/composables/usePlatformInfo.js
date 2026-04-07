import { ref } from 'vue';

export function usePlatformInfo() {
  const info = ref(null);

  async function load() {
    try {
      const response = await fetch('/api/info');
      if (!response.ok) return;
      info.value = await response.json();
    } catch {
      // ignore
    }
  }

  load();

  return { info };
}
