<script setup>
import { computed, ref, watch } from 'vue';
import { askLexostatus } from '../api/worldApi.js';
import LexostatusValues from '../components/LexostatusValues.vue';
import PersonaPicker from '../components/PersonaPicker.vue';
import { formatMoment, formatValue, humanize } from '../world/format.js';
import { cells, readAfwijzing, readLexostatus } from '../world/snapshot.js';
import { useWorld } from '../world/useWorld.js';

// Inzicht in je aanvraag: wat de cellen over deze aanvrager publiceren.
//
// Eén kaart per vraag uit het portaal. Elke vraag gaat naar één cel, langs
// dezelfde route als het tabblad Lexostatus, met de parameters die de server al
// voor deze persona invulde. Pas hier, in de weergave, komen de antwoorden naast
// elkaar te staan: combineren doet de consument (RFC-022 §4.1), en er wordt
// niets van vastgelegd. Een cel die nog niets vastgesteld heeft, zegt dat, en dat
// staat hier als "nog niets bekend" en niet als fout.
//
// De vragen gaan opnieuw zodra het beeld verandert — een aanvraag, een besluit,
// de klok die doorloopt — want dan kan het antwoord anders zijn. Welk beeld dat
// is, maakt niet uit: dezelfde store als op de andere pagina's.
//
// Een antwoord dat `decision_type` publiceert, en dat `AFWIJZING` is, leest als
// afwijzing: bovenaan de melding met haar gronden, daaronder wat de regeling wél
// uitrekende als "berekend, niet toegekend", en de vaste velden van het gram als
// gewone regels. Het gram draagt die bedragen bewust — het is wat de uitvoering
// opleverde — maar in één platte lijst lezen ze als een toekenning. Dit is
// platformvocabulaire en geen casus: elk decretogram draagt deze velden.

const { snapshot, portaal, persona, busy, error, actionError, choosePersona, dismissError } = useWorld();

/**
 * De antwoorden, in de volgorde van de vragen: per vraag
 * `{ loading, answer, error }`.
 */
const answers = ref([]);

/** Telt de rondes, zodat een trage ronde een nieuwere niet overschrijft. */
let round = 0;

async function askAll() {
  const vragen = persona.value?.inzicht ?? [];
  const current = ++round;
  answers.value = vragen.map(() => ({ loading: true, answer: null, error: null }));
  const results = await Promise.all(
    vragen.map(async (vraag) => {
      try {
        const payload = await askLexostatus(vraag.cell, vraag.lexostatus, vraag.params);
        return { loading: false, answer: readLexostatus(payload), error: null };
      } catch (cause) {
        return { loading: false, answer: null, error: cause?.message || 'De server gaf geen leesbare fout.' };
      }
    }),
  );
  if (current === round) answers.value = results;
}

/**
 * Per vraag de afwijzing, uitgesplitst; `null` waar het antwoord geen afwijzing
 * is.
 *
 * Bij de weergave en niet bij het vragen uitgerekend: wat een uitkomst is en wat
 * een vast veld, zegt de cel in het beeld, en dat hoort mee te bewegen met het
 * beeld dat er nu ligt.
 */
const afwijzingen = computed(() => {
  const vragen = persona.value?.inzicht ?? [];
  return answers.value.map((result, index) => {
    const cell = cells(snapshot.value).find((candidate) => candidate.id === vragen[index]?.cell);
    return readAfwijzing(result.answer, cell);
  });
});

/** Een afwijzingsgrond als regel eronder: de afwijzende waarde en het artikel. */
function grondDetail(grond) {
  return [`waarde: ${formatValue(grond.value)}`, grond.article ? `artikel ${grond.article}` : 'geen artikel bekend'].join(
    ' · ',
  );
}

watch([() => persona.value?.id, snapshot], askAll, { immediate: true });
</script>

<template>
  <nldd-split-view-pane slot="main" has-content>
    <nldd-page>
      <nldd-simple-section>
        <nldd-container layout="stack" gap="24">
          <nldd-title size="3">
            <span slot="overline">Aanvrager</span>
            <h1>Inzicht in je aanvraag</h1>
            <span slot="subtitle">wat elke cel over deze aanvrager publiceert, apart gevraagd</span>
          </nldd-title>

          <nldd-banner
            v-if="error && !actionError"
            variant="critical"
            text="De server kon dit niet doen"
            :supporting-text="error"
            dismissible
            @dismiss="dismissError()"
          ></nldd-banner>

          <PersonaPicker
            :portaal="portaal"
            :current="persona?.id ?? ''"
            :busy="busy"
            @choose="choosePersona($event)"
          />

          <nldd-inline-dialog
            v-if="!persona"
            icon="person"
            text="Kies als wie u kijkt"
            supporting-text="Dan staat hier wat de cellen over die aanvrager publiceren."
          ></nldd-inline-dialog>

          <template v-else>
            <nldd-card
              v-for="(vraag, index) in persona.inzicht"
              :key="`${persona.id}-${index}`"
              :accessible-label="vraag.label"
            >
              <nldd-container slot="header" layout="stack" gap="8" padding="16" padding-bottom="8">
                <nldd-title size="5">
                  <span slot="overline">{{ vraag.cell }} · {{ vraag.lexostatus }}</span>
                  <h2>{{ vraag.label }}</h2>
                  <span slot="subtitle">
                    op {{ formatMoment(answers[index]?.answer?.opMoment ?? snapshot?.clock) }}
                  </span>
                </nldd-title>
              </nldd-container>

              <nldd-container layout="stack" gap="12" padding="16" padding-top="8">
                <nldd-activity-indicator
                  v-if="!answers[index] || answers[index].loading"
                  show-text
                  text="De cel bevragen…"
                  size="24"
                ></nldd-activity-indicator>
                <nldd-banner
                  v-else-if="answers[index].error"
                  variant="critical"
                  text="Deze vraag kon niet gesteld worden"
                  :supporting-text="answers[index].error"
                ></nldd-banner>
                <!-- Een afwijzing: eerst de afwijzing en waarop ze afketste,
                     dan wat er wél uitgerekend is, en de vaste velden van het
                     gram als gewone regels. -->
                <template v-else-if="afwijzingen[index]">
                  <nldd-banner
                    variant="warning"
                    text="Afgewezen"
                    heading-level="3"
                    :supporting-text="
                      afwijzingen[index].gronden.length > 0
                        ? 'op grond van'
                        : 'het besluit noemt geen afwijzingsgrond'
                    "
                  >
                    <nldd-list
                      v-if="afwijzingen[index].gronden.length > 0"
                      variant="simple"
                      accessible-label="Afwijzingsgronden"
                    >
                      <nldd-list-item
                        v-for="(grond, position) in afwijzingen[index].gronden"
                        :key="`${grond.output}-${position}`"
                        size="sm"
                      >
                        <nldd-text-cell
                          size="sm"
                          :text="humanize(grond.output)"
                          :supporting-text="grondDetail(grond)"
                        ></nldd-text-cell>
                      </nldd-list-item>
                    </nldd-list>
                  </nldd-banner>
                  <template v-if="afwijzingen[index].berekend.values.length > 0">
                    <nldd-title size="6">
                      <h3>Berekend, niet toegekend</h3>
                      <span slot="subtitle">wat de regeling uitrekende; een afwijzing belooft er niets mee</span>
                    </nldd-title>
                    <LexostatusValues :answer="afwijzingen[index].berekend" />
                  </template>
                  <LexostatusValues
                    v-if="afwijzingen[index].vast.values.length > 0"
                    :answer="afwijzingen[index].vast"
                  />
                </template>
                <LexostatusValues
                  v-else-if="answers[index].answer.established"
                  :answer="answers[index].answer"
                />
                <nldd-inline-dialog
                  v-else
                  horizontal-alignment="left"
                  icon="info"
                  text="Nog niets bekend"
                  :supporting-text="answers[index].answer.reason"
                ></nldd-inline-dialog>
              </nldd-container>
            </nldd-card>
          </template>
        </nldd-container>
      </nldd-simple-section>
    </nldd-page>
  </nldd-split-view-pane>
</template>
