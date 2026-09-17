<script setup>
import { computed, ref, watch } from 'vue';
import ActionCard from '../components/ActionCard.vue';
import PersonaPicker from '../components/PersonaPicker.vue';
import { formatMoment } from '../world/format.js';
import { useWorld } from '../world/useWorld.js';

// Het aanvraagportaal: de wereld zoals een aanvrager haar ziet.
//
// Bovenaan kies je als wie je aanvraagt. Daaronder staan alleen de acties van de
// portaal-actor, met formulieren die de server al met de gegevens van die
// aanvrager invulde (`prefill` in het beeld). Deze pagina vult zelf niets in:
// wat de persona noemt, wint op de server van de gewone voorinvulling, en de
// kaart is dezelfde als op "Achter de schermen".
//
// Wat hier níet staat: de cellen, het journaal, wat andere actoren kunnen. Een
// aanvrager ziet dat niet, en daar gaat het om.

const { snapshot, portaal, persona, busy, error, actionError, act, choosePersona, dismissError } = useWorld();

/** De acties van de portaal-actor, zoals het beeld ze geeft. */
const actions = computed(() =>
  (snapshot.value?.actions ?? []).filter((action) => action.actor === portaal.value?.actor),
);

/**
 * Wat er net is ingediend: `{ label, moment }`, of `null`.
 *
 * Alleen voor deze aanvrager: wie wisselt, is iemand anders en heeft niets
 * ingediend.
 */
const submitted = ref(null);

watch(
  () => persona.value?.id,
  () => {
    submitted.value = null;
  },
);

async function run({ action, values }) {
  submitted.value = null;
  const next = await act(action, values);
  if (next) submitted.value = { label: action.label, moment: next.clock };
}

/** De melding van een actie staat bij haar kaart, dus niet nog eens bovenaan. */
const showError = computed(() => Boolean(error.value) && !actionError.value);

function errorOf(action) {
  return actionError.value?.action === action.id ? actionError.value.message : null;
}
</script>

<template>
  <nldd-split-view-pane slot="main" has-content>
    <nldd-page>
      <nldd-simple-section>
        <nldd-container layout="stack" gap="24">
          <nldd-title size="3">
            <span slot="overline">Aanvrager</span>
            <h1>{{ portaal.label }}</h1>
            <span slot="subtitle">wat u kunt doen, als de aanvrager die u hieronder kiest</span>
          </nldd-title>

          <nldd-banner
            v-if="showError"
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
            text="Kies als wie u aanvraagt"
            supporting-text="De formulieren worden dan ingevuld met de gegevens van die aanvrager. Er wordt niet ingelogd en er wordt niets vastgelegd."
          ></nldd-inline-dialog>

          <template v-else>
            <!-- De bevestiging, met de weg naar wat de cellen er nu over weten.
                 Een link en geen knop: het is een andere pagina. -->
            <nldd-banner
              v-if="submitted"
              variant="success"
              :text="`${submitted.label}: ingediend`"
              :supporting-text="`Op ${formatMoment(submitted.moment)}, als ${persona.label}.`"
              dismissible
              @dismiss="submitted = null"
            >
              <nldd-button
                slot="actions"
                variant="secondary"
                start-icon="search"
                text="Bekijk inzicht in je aanvraag"
                href="#/inzicht"
              ></nldd-button>
            </nldd-banner>

            <!-- Per aanvrager een eigen kaart: wat de vorige in een veld typte,
                 volgt de voorinvulling niet meer (zie ActionCard), en hoort na
                 een wissel niet onder de naam van de volgende te blijven staan. -->
            <ActionCard
              v-for="action in actions"
              :key="`${persona.id}:${action.id}`"
              :action="action"
              :snapshot="snapshot"
              :busy="busy"
              :error="errorOf(action)"
              @run="run"
            />

            <nldd-inline-dialog
              v-if="actions.length === 0"
              icon="hand"
              text="Geen acties"
              :supporting-text="`Het wereldbestand geeft '${portaal.actor}' geen acties.`"
            ></nldd-inline-dialog>
          </template>
        </nldd-container>
      </nldd-simple-section>
    </nldd-page>
  </nldd-split-view-pane>
</template>
