import { ref } from 'vue';
import { apiFetchJson } from '@regelrecht/frontend-shared';

const info = ref(null);
let fetched = false;

async function load() {
  try {
    info.value = await apiFetchJson('/api/harvest-admin/info');
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
