<script setup>
import { computed, ref } from 'vue';
import { humanize } from '../world/format.js';
import { besluitDefinitions, decretogramSchema } from '../world/snapshot.js';

// Wat een besluit van deze cel kan vastleggen, vóórdat er één besluit ligt: per
// veld van het decretogram het type en wie het veld declareert.
//
// Dit is een **meting** en geen weergave. Een decretogram krijgt zijn velden uit
// drie bronnen — de wet, het wereldbestand en het platform — en RFC-022 zegt dat
// normatieve inhoud in het lexogram hoort. Elk veld dat hier `wereldbestand`
// draagt is dus een **gat**: iets dat de wet zou moeten zeggen en dat nu in de
// configuratie van de opstelling staat. Die rijen staan er daarom anders bij, en
// de kop telt ze: wie een gat dicht, hoort dat getal te zien zakken.
//
// De rij klapt uit, net als een gram in de kroniek eronder: het schema is een
// tabel van vijftien rijen per besluit, en die horen niet standaard open te
// staan in een kolom die over de kronieken gaat.

const props = defineProps({
  /** De cel uit het beeld. */
  cell: { type: Object, required: true },
});

/** Welke besluit-rij open staat; leeg is alles dicht. */
const open = ref('');

const rows = computed(() =>
  besluitDefinitions(props.cell).map((definition) => {
    const schema = decretogramSchema(definition);
    const gaten = schema.filter((field) => field.gat).length;
    return {
      name: definition.name,
      zaakkenmerk: definition.zaakkenmerk ?? '',
      schema,
      gaten,
      // Wat de kop zegt zonder de tabel open te doen: hoe groot het gram is, en
      // hoeveel ervan uit de wet volgt.
      summary:
        schema.length === 0
          ? 'geen schema in dit beeld'
          : `${schema.length} velden · ${gaten} ${gaten === 1 ? 'gat' : 'gaten'}`,
    };
  }),
);

function toggle(name) {
  open.value = open.value === name ? '' : name;
}
</script>

<template>
  <nldd-container v-if="rows.length > 0" layout="stack" gap="4">
    <nldd-title size="6">
      <span>decretogram-schema</span>
      <span slot="subtitle">per veld: wie declareert het</span>
    </nldd-title>
    <nldd-list
      type="tree"
      variant="box-base"
      :accessible-label="`Decretogram-schema van cel ${cell.id}`"
      empty-text="Geen besluiten"
      empty-supporting-text="Deze cel kan niets besluiten."
    >
      <nldd-list-item
        v-for="row in rows"
        :key="`schema-${row.name}`"
        size="sm"
        button
        :expanded="open === row.name"
        @click="toggle(row.name)"
      >
        <nldd-icon-cell icon="certificate" size="16" color="secondary"></nldd-icon-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell size="sm" min-width="120px" :text="row.name" :supporting-text="row.summary"></nldd-text-cell>
        <!-- Het zaakkenmerk-sjabloon blijft bij het besluit dat het invult: dat
             is de vorm die de sleutel van de kroniek met beschikkingen krijgt. -->
        <nldd-cell v-if="row.zaakkenmerk">
          <nldd-tag size="sm" color="donkerblauw" :text="row.zaakkenmerk"></nldd-tag>
        </nldd-cell>
        <nldd-cell v-if="row.gaten > 0">
          <nldd-tag size="sm" color="rood" icon="warning" :text="`${row.gaten}`"></nldd-tag>
        </nldd-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>

        <nldd-list-item v-if="open === row.name" slot="children" size="sm">
          <nldd-spacer-cell size="20"></nldd-spacer-cell>
          <nldd-cell width="full" vertical-alignment="top">
            <nldd-table
              columns="minmax(160px, 1fr) fit-content(140px) minmax(180px, 1fr)"
              background="base"
              :accessible-label="`Schema van decretogram ${row.name}`"
              empty-text="Dit besluit kent geen velden"
            >
              <nldd-table-row slot="header">
                <nldd-text-cell size="sm" text="Veld"></nldd-text-cell>
                <nldd-text-cell size="sm" text="Type"></nldd-text-cell>
                <nldd-text-cell size="sm" text="Herkomst"></nldd-text-cell>
              </nldd-table-row>
              <nldd-table-row v-for="field in row.schema" :key="`${row.name}-${field.name}`">
                <!-- Een gat draagt zijn waarschuwing in de rij zelf en niet
                     alleen in de kleur van het label: kleur alleen is geen
                     onderscheid dat iedereen ziet. -->
                <nldd-text-cell
                  size="sm"
                  vertical-alignment="top"
                  :text="humanize(field.name)"
                  :supporting-text="field.toelichting"
                >
                  <nldd-tag
                    v-if="field.gat"
                    slot="overline"
                    size="sm"
                    color="rood"
                    icon="warning"
                    text="gat"
                  ></nldd-tag>
                </nldd-text-cell>
                <nldd-text-cell size="sm" vertical-alignment="top" :text="field.type"></nldd-text-cell>
                <nldd-text-cell size="sm" vertical-alignment="top" :supporting-text="field.lexogram">
                  <nldd-tag
                    size="sm"
                    :color="field.source.color"
                    :icon="field.source.icon"
                    :text="field.source.label"
                  ></nldd-tag>
                </nldd-text-cell>
              </nldd-table-row>
            </nldd-table>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list-item>
    </nldd-list>
  </nldd-container>
</template>
