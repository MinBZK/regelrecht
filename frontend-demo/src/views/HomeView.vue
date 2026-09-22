<script setup>
import { computed, onActivated, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useDemo } from '../store/demoStore.js';
import QrCode from '../components/QrCode.vue';
import { useI18n } from '../i18n/index.js';
import { localeRouteName } from '../router.js';

// De landingspagina op `/`. Wie de demo opent zonder te weten wat het is, leest
// hier in een paar regels wat er te zien valt, start de presentatie met één
// knop, en vindt de weg naar de rest van RegelRecht.
//
// De QR-code wijst naar deze pagina zelf, zodat iemand in de zaal de demo op
// zijn eigen telefoon kan openen terwijl hij naar het scherm kijkt.

const router = useRouter();
const { locale } = useI18n();

/** Het pad van een tabblad in de taal die aan staat. */
function pathFor(page) {
  return router.resolve({ name: localeRouteName(page, locale.value) }).path;
}
const { ready } = useDemo();

// De QR-code moet naar het adres wijzen waar déze pagina draait: productie,
// een preview-deploy of een laptop op het netwerk tijdens een presentatie. Die
// URL vastleggen zou op alle drie op één na fout zijn, dus hij komt uit de
// browser. `origin` en niet `href`: de route eronder verandert tijdens de demo
// mee, en de code hoort naar het beginpunt te leiden.
// Een computed en geen eenmalige `onMounted`: de view blijft door keep-alive
// gemount, dus een taalwissel moet de code meenemen. `origin` staat pas vast
// zodra er een window is, vandaar de ref eromheen.
const origin = ref('');
onMounted(() => {
  origin.value = window.location.origin;
});
const pageUrl = computed(() => {
  // Het pad van de voorpagina in de taal die aan staat, niet een vaste `/`:
  // wie tijdens een Engelse presentatie scant hoort in het Engels te landen.
  //
  // Expliciet 'home' en niet `route.name`: deze view blijft door keep-alive
  // gemount, dus zodra de presentator naar een ander tabblad loopt wijst
  // `route` daarheen en zou de code naar dat tabblad verwijzen. Bij het eerste
  // bezoek valt dat samen en daarom viel het niet op.
  if (!origin.value) return '';
  return `${origin.value}${pathFor('home')}`;
});

// Wat er in de demo te zien is, in de volgorde van de tabbladen erboven. Dit is
// een leeswijzer, geen tweede navigatie: de kaarten brengen je naar hetzelfde
// tabblad waar de tabbalk heen gaat.
const onderdelen = computed(() => [
  { icon: 'books', title: 'Wetten', text: 'De wet als machine-uitvoerbare YAML, naast de artikelen waar hij vandaan komt.', to: pathFor('wetten') },
  { icon: 'centralized-network', title: 'Graaf', text: 'Welke wet welke andere wet nodig heeft, en welke waarde daartussen loopt.', to: pathFor('graaf') },
  { icon: 'checklist', title: "Scenario's", text: 'Voorbeelden uit de memorie van toelichting, live doorgerekend door de engine.', to: pathFor('scenarios') },
  { icon: 'chart-line', title: 'Simulatie', text: 'Wat een regel doet bij een hele bevolking in plaats van bij één persoon.', to: pathFor('simulatie') },
  { icon: 'user', title: 'Mijn overheid', text: 'Hetzelfde corpus als portaal: waar heeft deze persoon recht op, en waarom.', to: pathFor('portaal') },
  { icon: 'inbox', title: 'Zaaksysteem', text: 'De andere kant van de balie: een behandelaar die een aanvraag beoordeelt.', to: pathFor('zaaksysteem') },
]);

const links = [
  { icon: 'home', title: 'regelrecht.rijks.app', text: 'Wat RegelRecht is, voor wie, en hoe je meedoet.', href: 'https://regelrecht.rijks.app' },
  { icon: 'document', title: 'Documentatie', text: 'Het wetformaat, de engine, de RFC’s en hoe je zelf een wet toevoegt.', href: 'https://docs.regelrecht.rijks.app/docs/' },
  { icon: 'library', title: 'Onderzoek', text: 'Het position paper Rules as Executed en het onderzoek eromheen.', href: 'https://regelrecht.rijks.app/research/' },
];

function start() {
  router.push(pathFor('presentatie'));
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
      <!-- Twee kolommen over de volle hoogte van de hero: het verhaal links, de
           QR-code rechts. In een zaal wordt die code in de eerste seconden
           gescand, dus hij hoort meteen in beeld en groot genoeg om vanaf een
           afstand te lezen. Op een smal scherm zakt hij onder de tekst. -->
      <div class="hero-row">
        <div class="hero-text">
          <nldd-title size="1" color="inherit">
            <span slot="overline">Demo</span>
            <h1>RegelRecht</h1>
            <span slot="subtitle">Van wet naar digitale werking</span>
          </nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-rich-text color="inherit">
            <p>
              Wat gebeurt er als de wet zelf machine-uitvoerbaar is en openbaar gepubliceerd wordt?
              Deze demo rekent het voor, in je eigen browser, op verzonnen personen.
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
              @click="router.push(pathFor('wetten'))"
            ></nldd-button>
          </nldd-button-group>
        </div>
        <QrCode
          v-if="pageUrl"
          class="qr"
          :value="pageUrl"
          accessible-label="QR-code naar deze pagina, om de demo op je telefoon te openen"
        />
      </div>
    </nldd-hero>

    <!-- Verder lezen staat boven de rondleiding: wie hier landt zonder de demo
         te kennen heeft eerder iets aan de weg naar RegelRecht zelf dan aan een
         beschrijving van tabbladen die vlak boven hem al staan. -->
    <nldd-simple-section>
      <nldd-title slot="header" size="3">
        <h2>Verder lezen</h2>
        <span slot="subtitle">Het werk waar deze demo uit voortkomt.</span>
      </nldd-title>
      <nldd-collection layout="grid" item-width="240px">
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
    </nldd-simple-section>

    <nldd-simple-section background="tinted">
      <nldd-title slot="header" size="3">
        <!-- Een kop die met "Wat ..." begint leest als een tussenkop uit een
             gegenereerde tekst; de sectie is een lijst van onderdelen, dus zij
             heet naar wat zij toont. -->
        <h2>De onderdelen</h2>
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
/* Tekst links, QR-code rechts, over de volle hoogte van de hero. Onder de
 * md-grens (1007px) stapelt het: de code zakt dan onder de tekst. */
.hero-row {
  display: flex;
  flex-wrap: wrap;
  align-items: stretch;
  gap: var(--primitives-space-32, 32px);
}

/* De tekst houdt een leesbare minimumbreedte; wordt het krapper, dan wikkelt de
   rij en zakt de code eronder. Dat is een eigenschap van de rij zelf en niet
   van het venster, dus geen media query: die keek naar de vensterbreedte en
   liet de kolom in een smalle container tot 54px samenknijpen. */
.hero-text {
  flex: 1 1 320px;
  min-width: min(100%, 320px);
}

/* De code draagt zijn eigen witte rand (de stille zone zit in de SVG), dus er
 * is geen kadertje omheen nodig.
 *
 * De maat komt van de BREEDTE, niet van de uitgerekte hoogte. `align-self:
 * stretch` samen met `aspect-ratio` gaf een lus: de code rekte tot de hoogte
 * van de rij, die hoogte bepaalde zijn breedte, en dat duwde de rij weer hoger
 * — de hero werd 5000px hoog en de tekstkolom 0 breed. */
.qr {
  flex: 0 0 auto;
  align-self: center;
  width: clamp(144px, 14vw, 240px);
  border-radius: var(--primitives-radius-8, 8px);
  overflow: hidden;
}

</style>
