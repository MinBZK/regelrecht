<template>
  <nldd-card>
    <nldd-container slot="header" padding="16" padding-bottom="0">
      <div class="ak-head">
        <span class="ak-title">{{ titel }}</span>
        <nldd-tag
          size="sm"
          :color="!entry.in_bestand ? 'grijs' : entry.aanvraag_vereist ? 'oranje' : 'groen'"
          :icon="!entry.in_bestand ? 'minus' : entry.aanvraag_vereist ? 'pencil-on-square' : 'check-mark-circle'"
          :text="!entry.in_bestand ? 'geen leerlingen in het bestand' : entry.aanvraag_vereist ? 'op aanvraag' : 'ambtshalve'"
        ></nldd-tag>
      </div>
    </nldd-container>
    <nldd-container padding="16">
      <nldd-banner v-if="entry.fallback" variant="warning">
        Schoolniveau-outputs ontbreken in dit corpus; bedragen zijn de som van de leerlingbedragen met een lokale drempeltoets.
      </nldd-banner>

      <dl class="ak-dl">
        <template v-if="sector === 'po'">
          <dt>Drempel (art. 34 lid 2)</dt>
          <dd>
            {{ entry.Ap + entry.Vp }} eerstejaars
            <nldd-tag size="sm" :color="entry.voldoet_aan_drempel ? 'success' : 'critical'" :text="entry.voldoet_aan_drempel ? 'gehaald' : 'niet gehaald'"></nldd-tag>
          </dd>
          <dt>Formulier 1: eerste opvang</dt>
          <dd>
            {{ entry.Ap }} asielzoeker{{ entry.Ap === 1 ? '' : 's' }}, {{ entry.Vp }} overige vreemdeling{{ entry.Vp === 1 ? '' : 'en' }}
            <span v-if="entry.onbeslist" class="ak-sub">, {{ entry.onbeslist }} nog te beoordelen</span>
            <span v-if="isEenJanuari(peildatum) && entry.At" class="ak-sub"><br />waarvan {{ entry.At }} al in de 1-februari-telling (At, lid 9: laag tarief)</span>
          </dd>
          <dt>Formulier 2: tweede jaar (art. 35)</dt>
          <dd>{{ entry.tweedejaars }} asielzoeker{{ entry.tweedejaars === 1 ? '' : 's' }}</dd>
          <template v-if="entry.aanvraag_vereist">
            <dt>Indienen bij DUO</dt>
            <dd v-if="entry.aanvraag">
              Bestand klaar op {{ datumLabel(bestandDatum(peildatum)) }}, uiterlijk <strong>{{ datumLabel(deadline) }}</strong>; te laat is afgewezen.
            </dd>
            <dd v-else class="ak-sub">Niets aan te vragen op deze peildatum.</dd>
          </template>
          <template v-else>
            <dt>DUO</dt>
            <dd>Berekent en betaalt uit ROD en de verblijfstitel, zonder aanvraag<span v-if="entry.ambigu">; {{ entry.ambigu }} leerling{{ entry.ambigu === 1 ? '' : 'en' }} met code 21/33/34/98 of zonder BSN blijven handwerk</span>.</dd>
          </template>
          <dt>Eerste opvang</dt>
          <dd>{{ euro(entry.bedrag_eerste_opvang) }}<span class="ak-sub"> · asielzoekers {{ euro(entry.per_categorie?.ASIELZOEKER) }} · overige {{ euro(entry.per_categorie?.OVERIGE_VREEMDELING) }}</span></dd>
          <dt>Tweede jaar</dt>
          <dd>{{ euro(entry.bedrag_tweede_jaar) }}</dd>
          <dt>Toeslag eerste keer (lid 7)</dt>
          <dd>{{ entry.toeslag_eerste_keer ? euro(entry.toeslag_eerste_keer) : '—' }}</dd>
        </template>

        <template v-else>
          <dt>Nieuwkomers op de peildatum</dt>
          <dd>{{ entry.nieuwkomers_vo }} <span class="ak-sub">· eerste categorie {{ entry.eerste_cat_vo }} · tweede categorie {{ entry.tweede_cat_vo }}</span></dd>
          <dt>Vaststelling</dt>
          <dd>DUO leest ROD op de zestiende na de peildatum en stelt de bekostiging ambtshalve vast in de maand erna; de accountant valideert.</dd>
          <dt>Bekostiging leerlingen</dt>
          <dd>{{ euro(entry.bedrag_leerlingen_vo) }}</dd>
          <dt>Voorbereidingskosten (op aanvraag, ≥ 10)</dt>
          <dd>{{ entry.voorbereidingskosten ? euro(entry.voorbereidingskosten) : '—' }}</dd>
        </template>

        <dt class="ak-totaal">Totaal deze peildatum</dt>
        <dd class="ak-totaal"><strong>{{ euro(entry.bedrag_totaal) }}</strong></dd>
      </dl>
    </nldd-container>
  </nldd-card>
</template>

<script setup>
import { euro } from '../../lib/format.js';
import { datumLabel, bestandDatum, isEenJanuari } from '../../lib/nieuwkomerFacts.js';

defineProps({
  titel: { type: String, required: true },
  entry: { type: Object, required: true },
  sector: { type: String, default: 'po' },
  peildatum: { type: String, required: true },
  deadline: { type: String, default: null },
});
</script>

<style scoped>
.ak-head { display: flex; align-items: center; justify-content: space-between; gap: var(--primitives-space-8); }
.ak-title { font-weight: 600; }
.ak-dl { margin: 0; display: grid; grid-template-columns: minmax(140px, 0.9fr) 1.6fr; gap: 6px 12px; font-size: 0.9em; }
.ak-dl dt { color: var(--semantics-content-secondary-color); }
.ak-dl dd { margin: 0; }
.ak-sub { color: var(--semantics-content-secondary-color); font-size: 0.9em; }
.ak-totaal { padding-top: 6px; border-top: 1px solid var(--semantics-dividers-color); }
</style>
