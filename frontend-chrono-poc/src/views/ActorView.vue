<script setup>
import { computed } from 'vue';
import ActionCard from '../components/ActionCard.vue';
import { fieldValue } from '../world/events.js';
import { actorHref } from '../world/route.js';
import { actionsByActor, cells } from '../world/snapshot.js';
import { useWorld } from '../world/useWorld.js';

// De pagina van één actor: wat deze actor kan doen, en wat het recht erover
// zegt.
//
// Dezelfde kaarten als op het portaal en achter de schermen, want het is
// dezelfde actie. Wat deze pagina toevoegt, is de blik: alleen de acties van
// déze actor, met bij elke actie de voorwaarden die haar eigen cel uitrekende —
// uit de wet die zij laadt, of uit haar eigen stand. Een voorwaarde die niet
// waar is, maakt de kaart anders en laat de knop staan: indienen kan altijd.
//
// Welke actor, staat in het adres (`#/actor/<cel>`); de keuzelijst zet het adres
// en de pagina volgt. Welke actoren er zijn, zegt het beeld: elke cel die in het
// wereldbestand een actie heeft.

const props = defineProps({
  /** De actor die open staat: een cel-id uit het beeld. */
  actor: { type: String, required: true },
});

const { snapshot, busy, error, actionError, act, dismissError } = useWorld();

const groups = computed(() => actionsByActor(snapshot.value));

/** De actoren om uit te kiezen, in de volgorde van het beeld. */
const actors = computed(() => groups.value.map((group) => group.actor));

/** De acties van deze actor. */
const actions = computed(() => groups.value.find((group) => group.actor === props.actor)?.actions ?? []);

/** De regelingen die de cel van deze actor laadt: daarmee rekent ze haar voorwaarden uit. */
const laws = computed(() => cells(snapshot.value).find((cell) => cell.id === props.actor)?.laws ?? []);

function choose(event) {
  const id = fieldValue(event, props.actor);
  if (id && id !== props.actor) window.location.hash = actorHref(id);
}

/** De melding van een actie staat bij haar kaart, dus niet nog eens bovenaan. */
const showError = computed(() => Boolean(error.value) && !actionError.value);

function errorOf(action) {
  return actionError.value?.action === action.id ? actionError.value.message : null;
}

function run({ action, values }) {
  return act(action, values);
}
</script>

<template>
  <nldd-split-view-pane slot="main" has-content>
    <nldd-page>
      <nldd-simple-section>
        <nldd-container layout="stack" gap="24">
          <nldd-title size="3">
            <span slot="overline">Actor</span>
            <h1>{{ actor }}</h1>
            <span slot="subtitle">wat deze actor kan doen, en wat de wet en de eigen stand erover zeggen</span>
          </nldd-title>

          <nldd-banner
            v-if="showError"
            variant="critical"
            text="De server kon dit niet doen"
            :supporting-text="error"
            dismissible
            @dismiss="dismissError()"
          ></nldd-banner>

          <nldd-form-field label="Actor" supporting-label="elke cel met acties in het wereldbestand">
            <nldd-dropdown width="360px" @change="choose">
              <select :value="actor">
                <option v-for="id in actors" :key="id" :value="id" :selected="id === actor">{{ id }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>

          <nldd-text size="sm" color="secondary">
            <template v-if="laws.length > 0">
              De voorwaarden rekent deze actor zelf uit, met wat zij laadt ({{ laws.join(', ') }}) en wat zij zelf
              weet. Wat zij niet weet, is onbekend: zij vraagt het niet aan een ander.
            </template>
            <template v-else>
              Deze actor laadt geen regelingen; een voorwaarde kan hier alleen haar eigen stand lezen.
            </template>
          </nldd-text>

          <ActionCard
            v-for="action in actions"
            :key="action.id"
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
            :supporting-text="`Het wereldbestand geeft '${actor}' geen acties.`"
          ></nldd-inline-dialog>
        </nldd-container>
      </nldd-simple-section>
    </nldd-page>
  </nldd-split-view-pane>
</template>
