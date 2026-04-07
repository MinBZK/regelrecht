import { ref } from 'vue';

const info = ref(null);
let fetched = false;

async function load() {
  try {
    const response = await fetch('/api/info');
    if (!response.ok) return;
    info.value = await response.json();
  } catch {
    // ignore
  }
}

export function usePlatformInfo() {
  if (!fetched) {
    fetched = true;
    load();
  }

  return { info };
}
