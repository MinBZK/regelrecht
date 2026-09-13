<script setup>
import { onActivated, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useDemo } from '../store/demoStore.js';
import QrCode from '../components/QrCode.vue';

// De landingspagina op `/`. Wie de demo opent zonder te weten wat het is, leest
// hier in een paar regels wat er te zien valt, start de presentatie met één
// knop, en vindt de weg naar de rest van RegelRecht.
//
// De QR-code wijst naar deze pagina zelf, zodat iemand in de zaal de demo op
// zijn eigen telefoon kan openen terwijl hij naar het scherm kijkt.

const router = useRouter();
const { ready } = useDemo();

// De QR-code moet naar het adres wijzen waar déze pagina draait: productie,
// een preview-deploy of een laptop op het netwerk tijdens een presentatie. Die
// URL vastleggen zou op alle drie op één na fout zijn, dus hij komt uit de
// browser. `origin` en niet `href`: de route eronder verandert tijdens de demo
// mee, en de code hoort naar het beginpunt te leiden.
const pageUrl = ref('');
onMounted(() => {
  pageUrl.value = `${window.location.origin}/`;
});

// Wat er in de demo te zien is, in de volgorde van de tabbladen erboven. Dit is
// een leeswijzer, geen tweede navigatie: de kaarten brengen je naar hetzelfde
// tabblad waar de tabbalk heen gaat.
const onderdelen = [
  { icon: 'books', title: 'Wetten', text: 'De wet als machine-uitvoerbare YAML, naast de artikelen waar hij vandaan komt.', to: '/wetten' },
  { icon: 'centralized-network', title: 'Graaf', text: 'Welke wet welke andere wet nodig heeft, en welke waarde daartussen loopt.', to: '/graaf' },
  { icon: 'checklist', title: "Scenario's", text: 'Voorbeelden uit de memorie van toelichting, live doorgerekend door de engine.', to: '/scenarios' },
  { icon: 'chart-line', title: 'Simulatie', text: 'Wat een regel doet bij een hele bevolking in plaats van bij één persoon.', to: '/simulatie' },
  { icon: 'user', title: 'Mijn overheid', text: 'Hetzelfde corpus als portaal: waar heeft deze persoon recht op, en waarom.', to: '/portaal' },
  { icon: 'inbox', title: 'Zaaksysteem', text: 'De andere kant van de balie: een behandelaar die een aanvraag beoordeelt.', to: '/zaaksysteem' },
];

const links = [
  { icon: 'home', title: 'regelrecht.rijks.app', text: 'Wat RegelRecht is, voor wie, en hoe je meedoet.', href: 'https://regelrecht.rijks.app' },
  { icon: 'document', title: 'Documentatie', text: 'Het wetformaat, de engine, de RFC’s en hoe je zelf een wet toevoegt.', href: 'https://docs.regelrecht.rijks.app' },
  { icon: 'library', title: 'Onderzoek', text: 'Het position paper Rules as Executed en het onderzoek eromheen.', href: 'https://regelrecht.rijks.app/#research' },
];

function start() {
  router.push('/presentatie');
}

// De scrollpositie van een keep-alive-view blijft staan. Voor een pagina waar
// iemand op landt is dat verkeerd: wie via het tabblad terugkomt hoort weer
// bovenaan te beginnen.
const page = ref(null);
onActivated(() => {
  page.value?.scrollTo?.({ top: 0 });
});
</script>

<template>
  <nldd-page ref="page">
    <!-- Het gekleurde vlak draagt zijn eigen contentkleur, dus titel, tekst en
         de inherit-knoppen erbinnen houden hun contrast zonder scheme-override. -->
    <nldd-hero main-width="full" main-background="donkerblauw" padding-bottom="0">
      <nldd-title size="1" color="inherit">
        <span slot="overline">Demo</span>
        <h1>RegelRecht</h1>
        <span slot="subtitle">Van wet naar digitale werking</span>
      </nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-rich-text color="inherit">
        <p>
          Wat gebeurt er als de wet zelf machine-uitvoerbaar is en openbaar gepubliceerd wordt?
          Deze demo rekent het voor, in uw eigen browser, op verzonnen personen.
        </p>
      </nldd-rich-text>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-button-group orientation="horizontal">
        <nldd-button
          size="lg"
          variant="inherit-filled"
          start-icon="play"
          text="Start de presentatie"
          :disabled="!ready || undefined"
          @click="start"
        ></nldd-button>
        <nldd-button
          size="lg"
          variant="inherit-tinted"
          start-icon="books"
          text="Zelf rondkijken"
          :disabled="!ready || undefined"
          @click="router.push('/wetten')"
        ></nldd-button>
      </nldd-button-group>
    </nldd-hero>

    <!-- Verder lezen staat boven de rondleiding: wie hier landt zonder de demo
         te kennen heeft eerder iets aan de weg naar RegelRecht zelf dan aan een
         beschrijving van tabbladen die vlak boven hem al staan.
         De QR-code hoort in deze sectie en niet in de volgende: in een zaal
         wordt hij vanaf de eerste seconde gescand, en hij mag daarvoor niet
         onder een rij kaarten weggezakt zijn. -->
    <nldd-two-thirds-one-third-section>
      <nldd-title slot="header" size="3">
        <h2>Verder lezen</h2>
        <span slot="subtitle">Het werk waar deze demo uit voortkomt.</span>
      </nldd-title>

      <nldd-collection slot="left" layout="grid" item-width="240px">
        <nldd-card v-for="l in links" :key="l.href" :href="l.href" target="_blank">
          <nldd-container padding="16" gap="8">
            <nldd-icon :name="l.icon" size="24"></nldd-icon>
            <nldd-title size="5">
              <h3>{{ l.title }}</h3>
            </nldd-title>
            <nldd-rich-text size="sm" spacing="tight">
              <p>{{ l.text }}</p>
            </nldd-rich-text>
          </nldd-container>
        </nldd-card>
      </nldd-collection>

      <nldd-box slot="right" background="tinted">
        <nldd-container padding="24" gap="16" horizontal-alignment="center">
          <nldd-title size="5">
            <h3>Kijk mee op uw telefoon</h3>
          </nldd-title>
          <div class="qr-frame">
            <QrCode v-if="pageUrl" :value="pageUrl" accessible-label="QR-code naar deze pagina" />
          </div>
        </nldd-container>
      </nldd-box>
    </nldd-two-thirds-one-third-section>

    <nldd-simple-section background="tinted">
      <nldd-title slot="header" size="3">
        <h2>Wat u hier kunt zien</h2>
        <span slot="subtitle">Dezelfde wetten, zes keer anders bekeken. De presentatie loopt er zelf langs.</span>
      </nldd-title>
      <nldd-collection layout="grid" item-width="240px">
        <nldd-card v-for="o in onderdelen" :key="o.to" button @click="router.push(o.to)">
          <nldd-container padding="16" gap="8">
            <nldd-icon :name="o.icon" size="24"></nldd-icon>
            <nldd-title size="5">
              <h3>{{ o.title }}</h3>
            </nldd-title>
            <nldd-rich-text size="sm" spacing="tight">
              <p>{{ o.text }}</p>
            </nldd-rich-text>
          </nldd-container>
        </nldd-card>
      </nldd-collection>
    </nldd-simple-section>
  </nldd-page>
</template>

<style scoped>
/* De QR-code heeft een witte ondergrond nodig om leesbaar te blijven, ook in
 * donkere modus. De code tekent zijn eigen witte vlak; dit kadertje geeft hem
 * een maximale maat en een radius die bij de rest van het vlak past. */
.qr-frame {
  width: 100%;
  max-width: 200px;
  padding: var(--primitives-space-8, 8px);
  border-radius: var(--primitives-radius-8, 8px);
  background: #fff;
}
</style>
