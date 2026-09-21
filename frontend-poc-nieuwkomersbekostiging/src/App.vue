<template>
  <nldd-app-view>
    <div class="app-frame">
      <nldd-skip-link text="Direct naar de inhoud">
      <nldd-top-navigation-bar
        logo-title="Nieuwkomersbekostiging"
        logo-subtitle="OCW/DUO-casus · knelpuntenbrief DUO 2025"
        :logo-href="b('/beleid')"
        :website-href="b('/beleid')"
        @click.prevent
      >
        <nldd-menu-bar slot="global">
          <nldd-menu-bar-item
            text="Beleid"
            icon="settings"
            :current="route.name === 'beleid' ? true : undefined"
            @click="router.push('/beleid')"
          ></nldd-menu-bar-item>
          <nldd-menu-bar-item
            text="Casussen"
            icon="user"
            :current="route.name === 'casussen' ? true : undefined"
            @click="router.push('/casussen')"
          ></nldd-menu-bar-item>
          <nldd-menu-bar-item
            text="School"
            icon="building"
            :current="route.name === 'school' ? true : undefined"
            @click="router.push('/school')"
          ></nldd-menu-bar-item>
        </nldd-menu-bar>
        <nldd-menu-bar slot="utility">
          <nldd-menu-bar-item
            :text="themaTekst"
            :icon="themaIcoon"
            @click="volgende"
          ></nldd-menu-bar-item>
        </nldd-menu-bar>
      </nldd-top-navigation-bar>
      <werkversie-balk />
      </nldd-skip-link>
      <main id="hoofdinhoud" class="app-main" tabindex="-1">
        <router-view />
      </main>
    </div>
  </nldd-app-view>
</template>

<script setup>
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useThema } from './composables/useThema.js';
import { b } from './basePad.js';
import WerkversieBalk from './components/WerkversieBalk.vue';

const route = useRoute();
const router = useRouter();
const { thema, volgende } = useThema();

// Het icoon toont wat je krijgt als je klikt, niet waar je nu staat: een
// knop die zijn eigen huidige stand afbeeldt, leest als een statuslampje.
const themaIcoon = computed(() => (thema.value === 'donker' ? 'light-mode' : 'dark-mode'));
const themaTekst = computed(() => (thema.value === 'donker' ? 'Licht' : 'Donker'));
</script>

<style scoped>
.app-frame {
  display: flex;
  flex-direction: column;
  /* nldd-app-view clipt zijn inhoud (overflow hidden in de shadow DOM);
     het kind moet zelf scrollen. Vaste hoogte + scrollende main. */
  height: 100vh;
}
.app-main {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
</style>
