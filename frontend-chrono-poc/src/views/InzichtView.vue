<script setup>
import { ref, watch } from 'vue';
import { askLexostatus } from '../api/worldApi.js';
import LexostatusValues from '../components/LexostatusValues.vue';
import PersonaPicker from '../components/PersonaPicker.vue';
import { formatMoment } from '../world/format.js';
import { readLexostatus } from '../world/snapshot.js';
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
