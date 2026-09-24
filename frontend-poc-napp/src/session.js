/** Shared session state: which roles are active in this browser session. */
import { reactive } from 'vue';
import { api } from './api.js';

export const session = reactive({
  loaded: false,
  aanvrager: null, // { kvk_nummer, partij_naam, machtiging: { type, orgaan?, gebied_code?, gebied_naam? } }
  beoordelaar: null, // { naam }
});

export async function refreshSession() {
  try {
    const me = await api.me();
    session.aanvrager = me.aanvrager;
    session.beoordelaar = me.beoordelaar;
  } catch {
    session.aanvrager = null;
    session.beoordelaar = null;
  }
  session.loaded = true;
}
