<template>
  <div class="gb">
    <h4 class="gb-kop">Je schuld en wat je betaalt</h4>
    <dl class="gb-raster">
      <div>
        <dt>Je studieschuld was</dt>
        <dd>{{ euroWhole(totals.startSchuld) }}</dd>
        <dd class="gb-uit">toen je begon met terugbetalen</dd>
      </div>
      <div>
        <dt>Rente erbij</dt>
        <dd>{{ euroWhole(totals.totaalRente) }}</dd>
        <dd class="gb-uit">rente over je schuld, in al die jaren bij elkaar</dd>
      </div>
      <div>
        <dt>Je betaalt zelf</dt>
        <dd>{{ euroWhole(totals.totaalBetaald) }}</dd>
        <dd class="gb-uit">{{ totals.totaalBetaald > 0 ? 'alles wat je in al die jaren overmaakt' : 'je inkomen is te laag om te betalen' }}</dd>
      </div>
      <div v-if="totals.kwijtgescholden > 0">
        <dt>Hoef je niet te betalen</dt>
        <dd>{{ euroWhole(totals.kwijtgescholden) }}</dd>
        <dd class="gb-uit">wat er in {{ totals.eindejaar }} nog staat; dat wordt kwijtgescholden</dd>
      </div>
      <div v-else-if="totals.restschuldBijOverlijden > 0">
        <dt>Blijft staan</dt>
        <dd>{{ euroWhole(totals.restschuldBijOverlijden) }}</dd>
        <dd class="gb-uit">je blijft betalen zolang je leeft</dd>
      </div>
    </dl>

    <p class="gb-som">
      Je schuld van {{ euroWhole(totals.startSchuld) }} plus {{ euroWhole(totals.totaalRente) }} rente is
      {{ euroWhole(totals.startSchuld + totals.totaalRente) }}.
      <template v-if="totals.totaalBetaald > 0 && totals.kwijtgescholden > 0">
        Daarvan betaal je {{ euroWhole(totals.totaalBetaald) }} zelf; de rest hoef je niet te betalen.
      </template>
      <template v-else-if="totals.kwijtgescholden > 0">
        Dat hele bedrag hoef je niet te betalen, omdat je inkomen daar te laag voor is.
      </template>
      <template v-else>
        Dat betaal je helemaal zelf.
      </template>
    </p>

    <h4 class="gb-kop">Welk inkomen gebruikt DUO?</h4>
    <dl class="gb-raster">
      <div>
        <dt>Peiljaar {{ peiljaar }}</dt>
        <dd>{{ euroWhole(inkomenPeiljaar) }}</dd>
        <dd class="gb-uit">hiermee rekent DUO je bedrag voor {{ startJaar }} uit</dd>
      </div>
      <div>
        <dt>Je inkomen nu ({{ startJaar }})</dt>
        <dd>{{ euroWhole(inkomenNu) }}</dd>
        <dd class="gb-uit">zoals het nu in deze berekening staat</dd>
      </div>
      <div v-if="persona.heeft_partner">
        <dt>Je partner verdient</dt>
        <dd>{{ euroWhole(persona.partnerinkomen ?? 0) }}</dd>
        <dd class="gb-uit">{{ partnerTelt ? 'dit telt mee bij je maandbedrag' : 'dit telt niet mee, dat heb je zelf gevraagd' }}</dd>
      </div>
    </dl>

    <nldd-banner v-if="verschiltVanPeiljaar" variant="accent">
      DUO rekent met je inkomen van twee jaar geleden ({{ peiljaar }}). Dat was
      {{ euroWhole(inkomenPeiljaar) }}, terwijl je nu {{ euroWhole(inkomenNu) }} verdient. Is je inkomen gedaald? Dan
      kun je peiljaarverlegging aanvragen, zodat DUO met een recenter jaar rekent.
    </nldd-banner>

    <h4 class="gb-kop">Wat DUO verder van je weet</h4>
    <dl class="gb-raster">
      <div>
        <dt>Huishouden</dt>
        <dd>{{ HUISHOUD_LABELS[persona.huishoudtype] ?? persona.huishoudtype }}</dd>
        <dd class="gb-uit">bepaalt hoeveel je mag houden voordat je gaat betalen</dd>
      </div>
      <div>
        <dt>Eerste studiefinanciering</dt>
        <dd>{{ String(persona.eerste_studiefinanciering).slice(0, 4) }}</dd>
        <dd class="gb-uit">samen met je opleiding bepaalt dit welke regels gelden</dd>
      </div>
      <div>
        <dt>Opleiding</dt>
        <dd>{{ ONDERWIJS_LABELS[persona.onderwijssoort] ?? persona.onderwijssoort }}</dd>
        <dd class="gb-uit">voor mbo gelden andere jaartallen dan voor hbo en universiteit</dd>
      </div>
      <div>
        <dt>Terugbetaalregime</dt>
        <dd>{{ regimeLabel(totals.regime) }}</dd>
        <dd class="gb-uit">de regels die voor jou gelden</dd>
      </div>
    </dl>
    <p class="gb-hint">
      Welke regels voor jou gelden, hangt af van wanneer je begon met studeren en welke opleiding je deed. Dat kun je
      niet zelf kiezen. Het bepaalt wel hoe lang je terugbetaalt en hoeveel je per maand betaalt.
    </p>
  </div>
</template>

<!--
  Waar het maandbedrag op gebaseerd is. Deze gegevens zaten al in de
  simulatie (startSchuld, totaalBetaald, totaalRente, inkomenInJaar) maar
  stonden nergens op het scherm, terwijl het precies de vragen zijn die een
  debiteur stelt: wat was mijn schuld, wat heb ik al betaald, en met welk
  inkomen rekent DUO eigenlijk?
-->

<script setup>
import { computed } from 'vue';
import { inkomenInJaar } from '../../sim/simulate.js';
import { euroWhole, regimeLabel } from '../../lib/format.js';

const props = defineProps({
  persona: { type: Object, required: true },
  totals: { type: Object, required: true },
  choices: { type: Object, default: () => ({}) },
  startJaar: { type: Number, default: 2026 },
});

const HUISHOUD_LABELS = {
  alleenstaand: 'Alleenstaand',
  alleenstaand_met_kind: 'Alleenstaand met kind',
  paar: 'Samenwonend of getrouwd',
  paar_met_kind: 'Samenwonend of getrouwd, met kind',
};

const ONDERWIJS_LABELS = { mbo: 'Mbo', hbo: 'Hbo', wo: 'Universiteit' };

// Peiljaar t-2: dat is waar de draagkracht op wordt bepaald (artikel 6.10).
const peiljaar = computed(() => props.startJaar - 2);
const inkomenPeiljaar = computed(() => inkomenInJaar(props.persona, props.startJaar, peiljaar.value));
const inkomenNu = computed(() => inkomenInJaar(props.persona, props.startJaar, props.startJaar));

const partnerTelt = computed(() => props.choices?.partnerMeetellen !== false);

/** Alleen melden als het echt scheelt; bij een vlakke inkomensgroei is het ruis. */
const verschiltVanPeiljaar = computed(() => {
  const p = inkomenPeiljaar.value;
  const n = inkomenNu.value;
  return p > 0 && Math.abs(n - p) / p > 0.05;
});
</script>

<style scoped>
.gb { display: flex; flex-direction: column; gap: var(--primitives-space-16); }
/* Nieuw onderwerp binnen hetzelfde blok: iets meer lucht erboven dan de
   gewone gap, zodat het als kop leest en niet als vierde kengetal. */
.gb-kop { margin: var(--primitives-space-8) 0 0; font-size: 1em; font-weight: 600; }
.gb-kop:first-child { margin-top: 0; }
/* De vier bedragen hierboven horen bij elkaar; deze regel maakt de optelsom
   expliciet, want anders lijkt "rente" een deel van "wat je betaalt". */
.gb-som {
  margin: 0;
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  font-size: 0.9em;
}
/* Vaste kolommen in plaats van auto-fit: de twee lijsten onder elkaar zijn
   losse dl's, en met auto-fit rekt een lijst met twee items zijn kolommen
   anders op dan de lijst met drie erboven. Dan staat "Je inkomen nu" niet
   onder "Daarvan betaal je". */
/* Eén raster voor alle drie de groepen, met dezelfde kolommaat, zodat de
   rijen onder elkaar uitlijnen in plaats van elk hun eigen indeling te
   kiezen. Elk item heeft dezelfde opbouw: label, waarde, toelichting. */
.gb-raster {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--primitives-space-16);
  margin: 0;
  align-items: start;
}
.gb-raster > div { display: flex; flex-direction: column; gap: var(--primitives-space-4); }
.gb-raster dt { font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.gb-raster dd { margin: 0; font-size: 1.3em; font-weight: 700; font-variant-numeric: tabular-nums; }
.gb-raster dd.gb-uit { font-size: 0.8em; font-weight: 400; color: var(--semantics-content-secondary-color); }
.gb-hint { margin: var(--primitives-space-8) 0 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>
