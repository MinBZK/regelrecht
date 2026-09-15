<template>
  <div class="pt">
    <p class="pt-hint">
      Gesimuleerde stand op 1 oktober (bekostigde nieuwkomers, huidig recht) naast de
      OCW-factsheetreeks, en de regeling-uitgaven naast de realisatie uit de jaarverslagen.
    </p>
    <nldd-table
      columns="80px repeat(6, minmax(110px, 1fr))"
      accessible-label="Plausibiliteit: simulatie naast factsheet en realisatie"
      empty-text="Nog geen simulatie"
    >
      <nldd-table-row slot="header">
        <nldd-text-cell size="sm" text="Jaar"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" text="po gesimuleerd" supporting-text="1 oktober"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" text="po factsheet"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" text="vo gesimuleerd" supporting-text="1 oktober"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" text="vo factsheet"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" text="uitgaven gesimuleerd" supporting-text="po + vo"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" text="realisatie" supporting-text="po + vo"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="r in rows" :key="r.jaar">
        <nldd-text-cell size="sm" :text="String(r.jaar)"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" :text="number(r.simPo)"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" :text="r.factPo != null ? number(r.factPo) : '—'" :supporting-text="afwijking(r.simPo, r.factPo)"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" :text="number(r.simVo)"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" :text="r.factVo != null ? number(r.factVo) : '—'" :supporting-text="afwijking(r.simVo, r.factVo)"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" :text="euroCompact(r.simUitgaven)"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" :text="r.realisatie != null ? euroCompact(r.realisatie) : '—'" :supporting-text="r.realisatieHint"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { number, euroCompact, percent } from '../../lib/format.js';

const props = defineProps({
  metrics: { type: Object, default: null },
  distributions: { type: Object, default: null },
});

const rows = computed(() => {
  const m = props.metrics;
  if (!m) return [];
  const stand = props.distributions?.stand_1_oktober ?? {};
  const real = props.distributions?.uitgaven_realisatie ?? {};
  return m.jaren.map((jaar) => {
    const y = m.perJaar[jaar];
    const po = real.po?.[jaar];
    const vo = real.vo?.[jaar];
    const realisatie = po != null || vo != null ? Number(po ?? 0) + Number(vo ?? 0) : null;
    const hint = realisatie == null ? '' : po == null ? 'alleen vo bekend' : vo == null ? 'alleen po bekend' : '';
    return {
      jaar,
      simPo: y.leerlingen_1_oktober.po,
      simVo: y.leerlingen_1_oktober.vo,
      factPo: stand[jaar]?.po ?? null,
      factVo: stand[jaar]?.vo ?? null,
      simUitgaven: y.regeling_totaal,
      realisatie,
      realisatieHint: hint,
    };
  });
});

function afwijking(sim, fact) {
  if (fact == null || !fact) return '';
  return `${sim >= fact ? '+' : '−'}${percent(Math.abs(sim - fact) / fact, 0)}`;
}
</script>

<style scoped>
.pt { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.pt-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>
