<script setup>
import { inject } from 'vue';

// De lege staat van de Machine- en YAML-panes, op één plek.
//
// Die twee panes tonen dezelfde gegevens (machine_readable, de een als
// formulier en de ander als YAML) en stonden daarom met dezelfde tekst en
// knoppen op drie plekken: MachineReadable, YamlView en de inline YAML-pane in
// EditorView. Ze liepen uit elkaar zodra er iets bijkwam. Eén component houdt
// ze gelijk.
//
// Drie toestanden, in volgorde van "wat vraagt nu je aandacht":
//   1. reviewReady - er ligt een voorstel klaar om te beoordelen
//   2. enriching   - er loopt een verrijking
//   3. leeg        - niets, dus bied de twee manieren aan om te beginnen
const props = defineProps({
  /** Er staat een openstaande review-taak klaar voor deze wet. */
  reviewReady: { type: Boolean, default: false },
  /** Het artikel waar die taak over gaat. Leeg bij een voorstel voor de hele
   *  wet. Staat in de tekst omdat de knop naar dat artikel navigeert, en dat
   *  hoeft niet het artikel te zijn dat je nu open hebt. */
  reviewArticle: { type: String, default: '' },
  /** Er loopt een verrijking voor deze wet (pending of processing). */
  enriching: { type: Boolean, default: false },
  /** Verrijken is hier aan te vragen (of routeert naar login/traject). */
  canEnrich: { type: Boolean, default: false },
  /** De machine-versie kan hier ter plekke worden aangemaakt (editor). */
  canWriteHere: { type: Boolean, default: false },
  /** Read-only context: bied een route naar de editor aan. */
  canCreate: { type: Boolean, default: false },
  /** Doel van die route; leeg laten zolang de gebruiker niet is ingelogd, dan
   *  gaat de klik door de login-popover in plaats van door de href. */
  createHref: { type: String, default: undefined },
  /** Melding van de laatste mislukte verrijk-aanvraag. */
  enrichError: { type: String, default: '' },
});

const emit = defineEmits([
  'enrich',
  /** Machine-versie hier ter plekke aanmaken. */
  'write',
  /** Naar de editor om hem daar aan te maken. Payload is het knopelement, zodat
   *  de ouder de login-popover eraan kan ankeren. */
  'create',
  'view-tasks',
  'review',
]);

const onLoginTriggerPointerdown = inject('onLoginTriggerPointerdown', () => {});
</script>

<template>
  <nldd-inline-dialog
    v-if="reviewReady"
    data-testid="review-ready"
    icon="ai"
    text="Er ligt een voorstel klaar"
    :supporting-text="reviewArticle
      ? `Beoordeel het voorstel voor artikel ${reviewArticle} en sla het op, of verwerp het.`
      : 'Beoordeel de wijzigingen en sla ze op, of verwerp ze.'"
  >
    <nldd-button
      slot="actions"
      variant="secondary"
      size="md"
      data-testid="review-btn"
      text="Beoordeel voorstel"
      @click="emit('review')"
    ></nldd-button>
  </nldd-inline-dialog>

  <nldd-inline-dialog
    v-else-if="enriching"
    data-testid="enriching"
    icon="ai"
    text="We genereren een voorstel"
    supporting-text="Er staat een wacht-taak klaar. Zodra het voorstel er is, kun je het beoordelen."
  >
    <nldd-button
      slot="actions"
      variant="secondary"
      size="md"
      data-testid="view-tasks-btn"
      text="Bekijk taken"
      @click="emit('view-tasks')"
    ></nldd-button>
  </nldd-inline-dialog>

  <nldd-inline-dialog
    v-else
    data-testid="no-machine-readable"
    text="Geen machine-leesbare gegevens voor dit artikel"
    :variant="enrichError ? 'alert' : undefined"
    :supporting-text="enrichError || (canEnrich ? 'Genereren verrijkt de hele wet en levert een voorstel per artikel op.' : undefined)"
  >
    <nldd-button
      v-if="canEnrich"
      slot="actions"
      variant="secondary"
      size="md"
      start-icon="ai"
      data-testid="enrich-btn"
      text="Genereer een voorstel"
      @click="emit('enrich', $event.currentTarget)"
      @pointerdown.capture="onLoginTriggerPointerdown"
    ></nldd-button>
    <nldd-button
      v-if="canWriteHere"
      slot="actions"
      variant="secondary"
      size="md"
      start-icon="write"
      data-testid="init-mr-btn"
      text="Stel handmatig op"
      @click="emit('write')"
    ></nldd-button>
    <nldd-button
      v-else-if="canCreate"
      slot="actions"
      variant="secondary"
      size="md"
      start-icon="write"
      data-testid="create-mr-btn"
      text="Stel handmatig op"
      :href="createHref"
      @click.prevent="emit('create', $event.currentTarget)"
      @pointerdown.capture="onLoginTriggerPointerdown"
    ></nldd-button>
  </nldd-inline-dialog>
</template>
