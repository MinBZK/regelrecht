<template>
  <nldd-app-view>
    <div class="app-frame">
      <nldd-skip-link text="Direct naar de inhoud">
      <nldd-top-navigation-bar
        logo-title="Terugbetalen studieschuld"
        logo-subtitle="OCW/DUO-casus · Stand van de Uitvoering 2026"
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
            text="Scenario's"
            icon="users"
            :current="route.name === 'scenarios' ? true : undefined"
            @click="router.push('/scenarios')"
          ></nldd-menu-bar-item>
          <nldd-menu-bar-item
            text="Burger"
            icon="user"
            :current="route.name === 'burger' ? true : undefined"
            @click="router.push('/burger')"
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

const themaIcoon = computed(
  () => ({ systeem: 'monitoring', licht: 'light-mode', donker: 'dark-mode' })[thema.value],
);
const themaTekst = computed(
  () => ({ systeem: 'Systeem', licht: 'Licht', donker: 'Donker' })[thema.value],
);
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
