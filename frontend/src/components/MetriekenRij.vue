<script setup>
/**
 * MetriekenRij: de vier tegels en de signalenlijst van de Analyse-pagina.
 *
 * Neemt het rapport van `corpusMetrics()` als prop en rekent zelf niets uit
 * behalve het percentage. Daardoor tonen het preview-harnas en de echte pagina
 * gegarandeerd hetzelfde, en is dit component zonder backend te testen.
 */
import { computed } from 'vue';

const props = defineProps({
  /** Uitvoer van `engine.corpusMetrics()`. */
  rapport: { type: Object, required: true },
  /** Wetten die de engine niet kon laden. Zelf een bevinding, geen ruis. */
  geweigerd: { type: Array, default: () => [] },
});

const tegels = computed(() => {
  const t = props.rapport?.totals;
  if (!t) return [];
  const dekking = t.articles > 0 ? Math.round((t.articles_with_logic / t.articles) * 100) : 0;
  return [
    {
      label: 'regelingen in het corpus',
      waarde: t.regulations,
      // Het aantal versies dat op de peildatum geldt tegenover alles wat
      // geladen is. Zonder dat tweede getal verdwijnt de geschiedenis uit
      // beeld: "22 regelingen, 22 versies" verzwijgt dat er 25 in het corpus
      // zitten.
      onder:
        t.versions_loaded > t.versions
          ? `${t.versions} geldend van ${t.versions_loaded} versies`
          : `${t.versions} versies`,
    },
    { label: 'artikelen opgenomen', waarde: t.articles, onder: `${t.articles_with_logic} met logica (${dekking}%)` },
    { label: 'machine-leesbare outputs', waarde: t.outputs, onder: `${t.parameters} parameters` },
    { label: 'untranslatables', waarde: t.untranslatables, onder: `${t.untranslatables_accepted} geaccepteerd` },
  ];
});

/**
 * Signalen die geen tegel verdienen maar wel gezien moeten worden.
 *
 * Een waarschuwing hangt alleen aan dingen die kapot zijn. Open terms en
 * niet-aangeroepen regelingen zijn de normale toestand van een corpus in
 * aanbouw; die een alarmkleur geven leert een lezer de kleur te negeren.
 */
const signalen = computed(() => {
  const m = props.rapport;
  if (!m?.totals) return [];
  const versies = [...new Set((m.regulations ?? []).map((r) => r.schema_version).filter(Boolean))].sort();
  const rijen = [
    {
      label: 'bindingen tussen regelingen',
      waarde: `${m.totals.bindings_clean} van ${m.totals.bindings} resolvend`,
      waarschuwing: m.totals.bindings_clean < m.totals.bindings,
    },
    { label: 'regelingen die niemand aanroept', waarde: String(m.totals.uncalled_regulations), waarschuwing: false },
    { label: 'open terms', waarde: String(m.totals.open_terms), waarschuwing: false },
    {
      label: 'schemaversies in gebruik',
      waarde: versies.join(', ') || 'geen',
      waarschuwing: versies.length > 1,
    },
  ];
  if (props.geweigerd.length) {
    rijen.push({
      label: 'wetten die de engine niet kon laden',
      waarde: String(props.geweigerd.length),
      waarschuwing: true,
    });
  }
  return rijen;
});
</script>

<template>
  <template v-if="tegels.length">
    <nldd-one-half-one-half-section>
      <nldd-title slot="header" size="6"><h3>Waar de vertaling staat</h3></nldd-title>
      <nldd-card v-for="(tegel, i) in tegels.slice(0, 2)" :key="tegel.label" :slot="i === 0 ? 'left' : 'right'">
        <nldd-container padding="16">
          <nldd-title size="2">
            <span slot="overline">{{ tegel.label }}</span>
            {{ tegel.waarde }}
            <span slot="subtitle">{{ tegel.onder }}</span>
          </nldd-title>
        </nldd-container>
      </nldd-card>
    </nldd-one-half-one-half-section>

    <nldd-one-half-one-half-section>
      <nldd-card v-for="(tegel, i) in tegels.slice(2)" :key="tegel.label" :slot="i === 0 ? 'left' : 'right'">
        <nldd-container padding="16">
          <nldd-title size="2">
            <span slot="overline">{{ tegel.label }}</span>
            {{ tegel.waarde }}
            <span slot="subtitle">{{ tegel.onder }}</span>
          </nldd-title>
        </nldd-container>
      </nldd-card>
    </nldd-one-half-one-half-section>

    <nldd-simple-section width="800px">
      <nldd-inline-dialog
        v-if="rapport.as_of"
        data-test="peildatum"
        :text="`Cijfers gelden op ${rapport.as_of}`"
        supporting-text="Een regeling telt mee met de versie die op deze datum in werking is."
      ></nldd-inline-dialog>
      <nldd-spacer v-if="rapport.as_of" size="12"></nldd-spacer>
      <nldd-list variant="simple">
        <nldd-list-item v-for="rij in signalen" :key="rij.label" data-test="signaal">
          <nldd-icon-cell v-if="rij.waarschuwing" size="20"><nldd-icon name="warning"></nldd-icon></nldd-icon-cell>
          <nldd-text-cell :text="rij.label"></nldd-text-cell>
          <nldd-text-cell :text="rij.waarde" horizontal-alignment="right"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>

      <template v-if="geweigerd.length">
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-banner
          variant="warning"
          data-test="geweigerd"
          :text="geweigerd.length === 1 ? 'Eén wet staat wel in het corpus maar niet in de engine' : `${geweigerd.length} wetten staan wel in het corpus maar niet in de engine`"
          :supporting-text="geweigerd.map((g) => `${g.lawId}: ${g.fout}`).join(' · ')"
        ></nldd-banner>
      </template>
      <nldd-spacer size="24"></nldd-spacer>
    </nldd-simple-section>
  </template>
</template>
