<template>
  <div class="ts">
    <p class="ts-intro">
      Je maandbedrag staat niet op zichzelf. Je inkomen en je huishouden bepalen ook wat je aan toeslagen krijgt, en
      een keuze die hier gunstig lijkt, kan daar geld kosten. Wat hieronder staat is <strong>niet</strong> doorgerekend:
      het laat zien welke regelingen naar dezelfde gegevens kijken.
    </p>

    <ul class="ts-lijst">
      <li v-for="r in regelingen" :key="r.naam">
        <span class="ts-naam">{{ r.naam }}</span>
        <span class="ts-waarom">{{ r.waarom }}</span>
        <nldd-tag size="sm" :color="r.kleur" :text="r.gegeven"></nldd-tag>
      </li>
    </ul>

    <nldd-banner v-if="partnerWaarschuwing" variant="warning">
      Je laat het inkomen van je partner niet meetellen voor je studieschuld. Voor de toeslagen telt dat inkomen wél
      mee: daar is je partner je toeslagpartner. Je maandbedrag gaat omlaag, je toeslagen niet omhoog.
    </nldd-banner>

    <p class="ts-voet">
      Wat je werkelijk overhoudt, hangt af van al deze regelingen samen. Een volledig beeld hoort in één dossier: welke
      beschikkingen er over jou zijn genomen en op welk inkomen die zijn gebaseerd. Die koppeling zit niet in deze
      demo.
    </p>
  </div>
</template>

<!--
  Signaalblok, geen rekenmachine. Afgesproken met Anne: de wisselwerking met
  toeslagen en andere schulden hoort in het beeld, maar we rekenen die
  regelingen hier niet door (dat zou betekenen dat de hele Awir, de
  zorgtoeslag en de huurtoeslag machine-uitvoerbaar moeten zijn). De route is
  later: bestaande beschikkingen ophalen bij de bronhouder en tonen, niet
  herberekenen. Zie de sectie "Buiten scope" in de README.
-->

<script setup>
import { computed } from 'vue';

const props = defineProps({
  persona: { type: Object, required: true },
  choices: { type: Object, default: () => ({}) },
});

const regelingen = computed(() => {
  const heeftKind = String(props.persona.huishoudtype ?? '').includes('kind');
  const lijst = [
    {
      naam: 'Zorgtoeslag',
      waarom: 'vervalt of daalt zodra je inkomen stijgt; met een toeslagpartner telt dat inkomen mee',
      gegeven: 'inkomen en partner',
      kleur: 'lintblauw',
    },
    {
      naam: 'Huurtoeslag',
      waarom: 'hangt af van je inkomen, je huur en wie er nog meer op je adres staat',
      gegeven: 'inkomen en huishouden',
      kleur: 'lintblauw',
    },
  ];
  if (heeftKind) {
    lijst.push({
      naam: 'Kindgebonden budget',
      waarom: 'daalt naarmate je huishoudinkomen stijgt',
      gegeven: 'huishoudinkomen',
      kleur: 'oranje',
    });
  }
  lijst.push({
    naam: 'Andere schulden',
    waarom: 'een lagere aflossing hier laat ruimte voor andere schuldeisers, en andersom',
    gegeven: 'afloscapaciteit',
    kleur: 'paars',
  });
  return lijst;
});

/**
 * De partner-opt-out is het scherpste voorbeeld van de wisselwerking: voor de
 * studieschuld telt het partnerinkomen dan niet mee, voor de toeslagen wel.
 */
const partnerWaarschuwing = computed(
  () => props.persona.heeft_partner && props.choices?.partnerMeetellen === false,
);
</script>

<style scoped>
.ts { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.ts-intro, .ts-voet { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.ts-lijst { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.ts-lijst li {
  display: flex;
  align-items: baseline;
  gap: var(--primitives-space-8);
  flex-wrap: wrap;
  padding-bottom: var(--primitives-space-8);
  border-bottom: 1px solid var(--semantics-dividers-color);
}
.ts-lijst li:last-child { border-bottom: none; padding-bottom: 0; }
.ts-naam { font-weight: 600; }
.ts-waarom { flex: 1 1 260px; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
</style>
