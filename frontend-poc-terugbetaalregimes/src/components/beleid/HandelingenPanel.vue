<template>
  <div class="hp">
    <nldd-banner v-if="!model" variant="warning">
      data/handelingen.yaml is niet geladen; de uitvoeringslast blijft leeg tot het bestand er is.
    </nldd-banner>

    <template v-else>
      <p class="hp-uitleg">
        Wat de uitvoering kost is het aantal keer dat een handeling voorkomt, maal de tijd die hij kost,
        maal een tarief. De aantallen komen uit de doorgerekende populatie; de minuten zijn aannames.
      </p>

      <details v-for="groep in groepen" :key="groep.partij" class="hp-group">
        <summary>
          {{ groep.label }}
          <nldd-badge size="sm" color="neutral" :number="groep.items.length" accessible-label="aantal handelingen"></nldd-badge>
          <span class="hp-som">{{ groep.som }}</span>
        </summary>
        <nldd-list variant="simple">
          <nldd-list-item v-for="h in groep.items" :key="h.id" size="sm">
            <nldd-text-cell
              size="sm"
              :text="`${h.minuten} min · ${aanleidingLabel(h.aanleiding)}`"
              :supporting-text="h.omschrijving"
            ></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
      </details>

      <details class="hp-group">
        <summary>Tarieven <span class="hp-som">€ per uur</span></summary>
        <nldd-list variant="simple">
          <nldd-list-item v-for="(bedrag, naam) in model.tarieven" :key="naam" size="sm">
            <nldd-text-cell size="sm" :text="`${naam}: € ${Math.round(bedrag / 100)}`" supporting-text="HOT 2026, integrale kosten per salarisschaal"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
      </details>

      <p class="hp-bron">
        De tijd van debiteuren telt in uren en niet in euro's: er bestaat geen tarief voor de tijd van een
        burger. Zonder die scheiding zou een regeling die werk naar de aanvrager verschuift goedkoper lijken
        dan hij is.
      </p>
    </template>
  </div>
</template>

<!--
  Het uitvoeringslastmodel, ter inzage.

  Nieuwkomersbekostiging heeft hier een bewerkbare variant van, waarin je
  minuten en tarieven live kunt bijstellen. Dat hangt daar aan een
  patch-laag die het model in de sessie overschrijft (useHandelingen plus
  handelingenPatch, samen ~500 regels). Hier staat eerst de leesbare versie:
  elke aanname is zichtbaar met zijn bron, wat de vraag beantwoordt waar deze
  getallen vandaan komen. Bewerkbaar maken kan daarna, als OCW eraan wil
  draaien.
-->

<script setup>
import { computed } from 'vue';
import { usePopulation } from '../../composables/usePopulation.js';
import { euroCompact, number } from '../../lib/format.js';

const { handelingenModel: model, metrics } = usePopulation();

const PARTIJ_LABEL = {
  duo: 'DUO',
  debiteur: 'De debiteur',
};

const AANLEIDING_LABEL = {
  per_aflossende_debiteur: 'per aflossende debiteur, per jaar',
  per_draagkrachtmeting: 'per aangevraagde draagkrachtmeting',
  per_partner_opt_out: 'per partner-opt-out',
  per_debiteur_betalingsprobleem: 'per debiteur met een betalingsprobleem',
};

function aanleidingLabel(a) {
  return AANLEIDING_LABEL[a] ?? a;
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
</style>
