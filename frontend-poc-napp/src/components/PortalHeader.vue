<script setup>
/**
 * Gedeelde kop voor alle drie de ingangen: rijkslogo-navigatiebalk
 * (nldd-top-navigation-bar) met portaal-specifieke menu-items.
 * Items met `to` navigeren binnen het portaal (SPA); items met `href`
 * verlaten het portaal (volledige paginalading).
 */
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { api } from '../api.js';
import { session, refreshSession } from '../session.js';
import PocBanner from './PocBanner.vue';

// Functionele sitenaam onder het woordmerk, per portaal vast; afgeleid
// van de portal-prop zodat hij nooit per pagina kan verschillen.
const PORTAL_SUBTITLES = {
  aanvrager: 'Subsidieportaal',
  beoordelaar: 'Beoordelingsomgeving',
  publiek: 'Publieksportaal',
};

const props = defineProps({
  items: { type: Array, default: () => [] }, // { text, to? , href?, icon? }
  // 'aanvrager' | 'beoordelaar' | null (publiek): toont rechtsboven wie is
  // ingelogd, op elke pagina van het portaal.
  portal: { type: String, default: null },
  utilityItems: { type: Array, default: () => [] },
});
const emit = defineEmits(['utility']);

const router = useRouter();
const route = useRoute();

const sessieItems = computed(() => {
  if (props.portal === 'aanvrager' && session.aanvrager) {
    const machtiging = session.aanvrager.machtiging;
    if (machtiging?.type === 'BEPERKT') {
      // Branch login: show who is represented plus a subtle scope marker.
      return [
        {
          text: `${session.aanvrager.partij_naam} · afdeling ${machtiging.gebied_naam} (beperkte machtiging)`,
          icon: 'person',
          key: 'machtiging-info',
        },
        { text: 'Uitloggen', icon: 'logout', key: 'logout-aanvrager' },
      ];
    }
    return [
      {
        text: `Uitloggen (${session.aanvrager.partij_naam})`,
        icon: 'logout',
        key: 'logout-aanvrager',
      },
    ];
  }
  if (props.portal === 'beoordelaar' && session.beoordelaar) {
    return [{ text: session.beoordelaar.naam, icon: 'person', key: 'beoordelaar' }];
  }
  return [];
});

const alleUtilityItems = computed(() => [...props.utilityItems, ...sessieItems.value]);

async function onUtility(item) {
  if (item.key === 'machtiging-info') return; // informational, no action
  if (item.key === 'logout-aanvrager') {
    await api.eherkenningLogout();
    await refreshSession();
    router.push('/');
    return;
  }
  emit('utility', item);
}

function isCurrent(item) {
  if (!item.to) return false;
  if (item.to === '/') return route.path === '/';
  return route.path.startsWith(item.to);
}

function onSelect(event, item) {
  if (item.to) {
    event.preventDefault();
    router.push(item.to);
  }
}
</script>

<template>
  <PocBanner />
  <nldd-skip-link text="Direct naar de inhoud">
    <!-- Rijkshuisstijl: het woordmerk naast het beeldmerk is de officiële
         organisatienaam, op elk portaal identiek; de functionele sitenaam
         staat als vaste tweede regel eronder. -->
    <nldd-top-navigation-bar
      logo-title="Nederlandse autoriteit politieke partijen"
      :logo-subtitle="PORTAL_SUBTITLES[portal] ?? ''"
      logo-href="/"
      website-href="/"
    >
      <nldd-menu-bar v-if="items.length" slot="global">
        <nldd-menu-bar-item
          v-for="item in items"
          :key="item.text"
          :text="item.text"
          :href="item.href ?? item.to"
          :icon="item.icon"
          :current="isCurrent(item) || undefined"
          @click="onSelect($event, item)"
        ></nldd-menu-bar-item>
      </nldd-menu-bar>
      <nldd-menu-bar v-if="alleUtilityItems.length" slot="utility">
        <nldd-menu-bar-item
          v-for="item in alleUtilityItems"
          :key="item.text"
          :text="item.text"
          :icon="item.icon"
          @click="onUtility(item)"
        ></nldd-menu-bar-item>
      </nldd-menu-bar>
    </nldd-top-navigation-bar>
  </nldd-skip-link>
</template>
