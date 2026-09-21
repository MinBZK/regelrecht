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
import { b } from '../basePad.js';

// Functionele sitenaam, per portaal vast; afgeleid van de portal-prop zodat
// hij nooit per pagina kan verschillen. Het aanvragersportaal voert hem als
// websitetitel boven het menu, de andere nog onder het woordmerk.
const PORTAL_SUBTITLES = {
  beoordelaar: 'Beoordelingsomgeving',
  publiek: 'Publieksportaal',
};
const WEBSITE_TITLES = {
  aanvrager: 'Subsidieportaal',
};

const props = defineProps({
  items: { type: Array, default: () => [] }, // { text, to? , href?, icon? }
  // 'aanvrager' | 'beoordelaar' | null (publiek): toont rechtsboven wie is
  // ingelogd, op elke pagina van het portaal.
  portal: { type: String, default: null },
  utilityItems: { type: Array, default: () => [] },
  back: { type: Object, default: null }, // { text, to }
});
const emit = defineEmits(['utility']);

const router = useRouter();
const route = useRoute();

const aanvrager = computed(() =>
  props.portal === 'aanvrager' ? session.aanvrager : null,
);

// Branch login: the identity names the represented party plus the scope.
const machtigingTekst = computed(() => {
  const machtiging = aanvrager.value?.machtiging;
  return machtiging?.type === 'BEPERKT'
    ? `Afdeling ${machtiging.gebied_naam} (beperkte machtiging)`
    : '';
});

const sessieItems = computed(() => {
  if (props.portal === 'beoordelaar' && session.beoordelaar) {
    return [{ text: session.beoordelaar.naam, icon: 'person', key: 'beoordelaar' }];
  }
  return [];
});

const alleUtilityItems = computed(() => [...props.utilityItems, ...sessieItems.value]);

function onUtility(item) {
  emit('utility', item);
}

async function logoutAanvrager() {
  await api.eherkenningLogout();
  await refreshSession();
  router.push('/');
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

// Het menu-item rendert een echte <a href>. Links klikken vangt `onSelect` af,
// maar middelklik, "openen in nieuw tabblad" en de statusbalk lezen het pad
// zoals het er staat. Een `to` is relatief aan de router-basis, niet aan de
// app-basis, dus `b()` volstaat hier niet: onder /napp/ moet `/partijregister`
// van de beoordelingsomgeving /napp/beoordelaar/partijregister worden. `router.resolve` kent die
// basis al en levert het volledige pad.
function itemHref(item) {
  if (item.href) return item.href;
  return item.to ? router.resolve(item.to).href : undefined;
}
</script>

<template>
  <!-- Geen eigen voorbehoud-balk meer: het portaal injecteert er al een boven
       elke pagina van een poc (packages/poc-portal, `voorbehoud_strip`), met
       de casusnaam en de status erbij. Twee balken onder elkaar die hetzelfde
       zeggen lezen als een fout, en de bovenste is de inhoudelijke. Napp is
       als losse poc begonnen, vandaar dat hij er zelf een had. -->
  <nldd-skip-link text="Direct naar de inhoud">
    <!-- Rijkshuisstijl: het woordmerk naast het beeldmerk is de officiële
         organisatienaam, op elk portaal identiek; de functionele sitenaam
         staat als vaste tweede regel eronder. -->
    <nldd-top-navigation-bar
      logo-title="Nederlandse autoriteit politieke partijen"
      :logo-subtitle="PORTAL_SUBTITLES[portal] ?? ''"
      :logo-href="b('/')"
      :website-title="WEBSITE_TITLES[portal] ?? ''"
      :website-href="WEBSITE_TITLES[portal] ? router.resolve('/').href : b('/')"
      :back-text="back?.text"
      @back-click="router.push(back.to)"
    >
      <nldd-menu-bar v-if="items.length" slot="global">
        <nldd-menu-bar-item
          v-for="item in items"
          :key="item.text"
          :text="item.text"
          :href="itemHref(item)"
          :icon="item.icon"
          :current="isCurrent(item) || undefined"
          @click="onSelect($event, item)"
        ></nldd-menu-bar-item>
      </nldd-menu-bar>
      <nldd-menu-bar v-if="alleUtilityItems.length || aanvrager" slot="utility">
        <nldd-menu-bar-item
          v-for="item in alleUtilityItems"
          :key="item.text"
          :text="item.text"
          :icon="item.icon"
          @click="onUtility(item)"
        ></nldd-menu-bar-item>
        <nldd-menu-bar-item v-if="aanvrager" text="Account" icon="account" expandable>
          <nldd-menu>
            <nldd-container slot="header" padding-inline="16" padding-block="12">
              <nldd-identity
                :text="aanvrager.partij_naam"
                :supporting-text="machtigingTekst"
              ></nldd-identity>
            </nldd-container>
            <nldd-menu-item
              text="Mijn organisatie"
              icon="building"
              :href="router.resolve('/organisatie').href"
              @click="onSelect($event, { to: '/organisatie' })"
            ></nldd-menu-item>
            <nldd-menu-divider></nldd-menu-divider>
            <nldd-menu-item text="Uitloggen" icon="logout" @select="logoutAanvrager"></nldd-menu-item>
          </nldd-menu>
        </nldd-menu-bar-item>
      </nldd-menu-bar>
    </nldd-top-navigation-bar>
  </nldd-skip-link>
</template>
