<template>
  <div class="hp">
    <nldd-banner v-if="!basis" variant="warning">
      data/handelingen.yaml is niet geladen; de uitvoeringslast blijft leeg tot het bestand er is.
    </nldd-banner>

    <template v-else>
      <div class="hp-kop">
        <p class="hp-uitleg">
          Wat de uitvoering kost is het aantal keer dat een handeling voorkomt, maal de tijd die hij kost,
          maal een tarief. De aantallen komen uit de doorgerekende populatie; de minuten zijn aannames die
          je hier kunt bijstellen.
        </p>
        <nldd-button
          v-if="gewijzigd"
          size="sm"
          text="Herstel het model"
          start-icon="undo"
          variant="neutral-transparent"
          @click="herstelHandelingen"
        ></nldd-button>
      </div>

      <details v-for="groep in groepen" :key="groep.partij" class="hp-group">
        <summary>
          {{ groep.label }}
          <nldd-badge size="sm" color="neutral" :number="groep.items.length" accessible-label="aantal handelingen"></nldd-badge>
          <span class="hp-som">{{ groep.som }}</span>
        </summary>
        <div class="hp-velden">
          <nldd-form-field
            v-for="h in groep.items"
            :key="h.id"
            :label="`${aanleidingLabel(h.aanleiding)} (minuten)`"
            :supporting-label="h.omschrijving"
          >
            <!-- Op `input` en niet op `change`: dat laatste event draagt niet
                 altijd een detail, en dan zou de waarde stil verdwijnen. De
                 uitkomst rekent mee terwijl je typt, wat hier juist het punt is. -->
            <nldd-number-field
              :value="minutenVan(h)"
              :min="0"
              :step="5"
              size="sm"
              @input="stelMinutenIn(h.id, $event.detail?.value)"
            ></nldd-number-field>
          </nldd-form-field>
        </div>
      </details>

      <details class="hp-group">
        <summary>
          Tarieven
          <nldd-badge size="sm" color="neutral" :number="tariefNamen.length" accessible-label="aantal tarieven"></nldd-badge>
          <span class="hp-som">€ per uur</span>
        </summary>
        <div class="hp-velden">
          <nldd-form-field
            v-for="naam in tariefNamen"
            :key="naam"
            :label="`${naam} (€ per uur)`"
            supporting-label="HOT 2026, integrale kosten per salarisschaal"
          >
            <!-- Het model rekent in eurocent; hier staan hele euro's, want zo
                 staat het ook in de HOT. -->
            <nldd-number-field
              :value="Math.round(tariefVan(naam) / 100)"
              :min="0"
              :step="5"
              size="sm"
              @input="stelTariefInEuro(naam, $event.detail?.value)"
            ></nldd-number-field>
          </nldd-form-field>
        </div>
      </details>

      <p class="hp-bron">
        De tijd van debiteuren telt in uren en niet in euro's: er bestaat geen tarief voor de tijd van een
        burger. Zonder die scheiding zou een regeling die werk naar de aanvrager verschuift goedkoper lijken
        dan hij is.
      </p>
      <p v-if="gewijzigd" class="hp-bron">
        <strong>Bijgesteld voor dit gesprek.</strong> Deze wijziging gaat niet mee naar een variant en is na
        een ververs weg; het model in het dossier blijft zoals het is.
      </p>
    </template>
  </div>
</template>

<!--
  Het uitvoeringslastmodel: elke aanname zichtbaar, en bij te stellen.

  Elke minuut in handelingen.yaml is een aanname en staat daar ook zo
  gemarkeerd. Dan hoort er ook aan te draaien te zijn, want de vraag van een
  uitvoeringsorganisatie is niet "klopt dit getal" maar "wat gebeurt er als het
  de helft is". Bijstellen kost geen hersimulatie: de aantallen komen uit de
  populatie, en minuten maal tarief is rekenwerk op de hoofdthread.

  De bijstellingen zijn bewust niet bewaard. Ze horen bij één gesprek, niet bij
  het dossier, en na een ververs staat het cijfer er weer zoals iedereen het
  kent.
-->

<script setup>
import { computed } from 'vue';
import { usePopulation } from '../../composables/usePopulation.js';
import { euroCompact, number } from '../../lib/format.js';

const {
  handelingenBasis: basis,
  handelingenModel: model,
  handelingenGewijzigd: gewijzigd,
  stelMinutenIn,
  stelTariefIn,
  herstelHandelingen,
  metrics,
} = usePopulation();

const PARTIJ_LABEL = {
  duo: 'DUO',
  debiteur: 'De debiteur',
};

const AANLEIDING_LABEL = {
  per_aflossende_debiteur: 'Per aflossende debiteur, per jaar',
  per_draagkrachtmeting: 'Per aangevraagde draagkrachtmeting',
  per_partner_opt_out: 'Per partner-opt-out',
  per_debiteur_betalingsprobleem: 'Per debiteur met een betalingsprobleem',
};

function aanleidingLabel(a) {
  return AANLEIDING_LABEL[a] ?? a;
}

/** De minuten zoals er nu mee gerekend wordt (bijgesteld of uit het dossier). */
function minutenVan(h) {
  return model.value?.handelingen?.find((x) => x.id === h.id)?.minuten ?? h.minuten;
}

const tariefNamen = computed(() => Object.keys(model.value?.tarieven ?? {}));

function tariefVan(naam) {
  return Number(model.value?.tarieven?.[naam] ?? 0);
}

/**
 * Het veld toont hele euro's, het model rekent in eurocent. Een leeg veld
 * stuurt niets mee; dat mag geen tarief van nul worden, want dan zou de
 * uitvoering gratis lijken zolang iemand aan het typen is.
 */
function stelTariefInEuro(naam, euro) {
  if (euro === null || euro === undefined) return;
  stelTariefIn(naam, Math.round(Number(euro) * 100));
}

/**
 * Per partij de handelingen, met het totaal dat deze partij in de huidige
 * doorrekening draagt. DUO in euro's, de debiteur in uren: dat verschil is de
 * kern van het model en hoort dus ook in de kop van elke groep te staan.
 */
const groepen = computed(() => {
  if (!model.value?.handelingen) return [];
  const last = metrics.value?.uitvoeringslast ?? null;
  const perPartij = new Map();
  for (const h of model.value.handelingen) {
    if (!perPartij.has(h.partij)) perPartij.set(h.partij, []);
    perPartij.get(h.partij).push(h);
  }
  return [...perPartij].map(([partij, items]) => {
    const p = last?.perPartij?.[partij];
    return {
      partij,
      label: PARTIJ_LABEL[partij] ?? partij,
      items,
      som: !p ? '' : p.inGeld ? euroCompact(p.kosten) : `${number(Math.round(p.uren))} uur`,
    };
  });
});
</script>

<style scoped>
.hp { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.hp-uitleg, .hp-bron { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
/* De herstelknop rechts naast de uitleg, zodat hij niet als losse regel
   boven het paneel zweeft. */
.hp-kop { display: flex; align-items: flex-start; gap: var(--primitives-space-12); }
.hp-kop > .hp-uitleg { flex: 1 1 auto; }
.hp-group {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
}
.hp-group > summary {
  display: flex; align-items: center; gap: var(--primitives-space-8);
  padding: var(--primitives-space-8) var(--primitives-space-12);
  cursor: pointer; font-weight: 600;
}
/* Het totaal van de partij staat rechts uitgelijnd, zodat de twee eenheden
   (euro's bij DUO, uren bij de debiteur) onder elkaar te vergelijken zijn. */
.hp-som {
  margin-left: auto;
  font-weight: 400; font-variant-numeric: tabular-nums;
  color: var(--semantics-content-secondary-color);
}
.hp-velden {
  display: flex; flex-direction: column; gap: var(--primitives-space-12);
  padding: var(--primitives-space-12);
}
</style>
