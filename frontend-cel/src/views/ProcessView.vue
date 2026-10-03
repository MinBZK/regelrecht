<script setup>
// A process. Which roles there are, through which channel they log in and
// which screens they have, GET /api/processes says (the channels of the
// actor's policy, RFC-047): a role with routes portal sees what the policy offers
// and submits, a role with routes handling sees the worklist and all cases
// (also a decided one, for what follows the decision), a case with
// its actions (the decision, the publication, a payment, a fact from its
// course), a role with routes counter enters an application that came in
// some other way. The chronicle and the lexostatuses belong to the cell the
// process records in; only a role with routes handling sees them, through
// the inspection of the process. Whoever submits sees the gram of their own
// submission.
import { computed, onMounted, provide, ref } from 'vue';
import { inspectionApi, processApi } from '../api.js';
import { fragmentCache } from '../why.js';
import { rolesOf, sessionText, startScreen as startScreenOf } from '../channel.js';
import LoginView from './LoginView.vue';
import PossibilitiesView from './PossibilitiesView.vue';
import ApplicationView from './ApplicationView.vue';
import CounterView from './CounterView.vue';
import ChronicleView from './ChronicleView.vue';
import LexostatusView from './LexostatusView.vue';
import WorklistView from './WorklistView.vue';
import CaseView from './CaseView.vue';
import Grams from '../components/Grams.vue';

const props = defineProps({
  process: { type: Object, required: true },
  // The cell the process records in, as GET /api/cells describes it.
  cell: { type: Object, required: true },
});

const api = processApi(props.process.id);
provide('api', api);
// The YAML fragments behind the steps of the "waarom?", fetched once each
// per process, so they survive a remount of the form.
provide('fragment', fragmentCache((path) => api.fragment(path)));
provide('cellApi', inspectionApi(props.process.id, props.cell.id));
// The examples of the process (logins, application, and a form per action);
// without them: empty.
const examples = ref({ logins: [], application: null, actions: {} });
provide('examples', examples);
provide('process', props.process);

const roles = computed(() => rolesOf(props.process));
const role = ref(roles.value[0]?.id ?? null);
// One session per process: logging in with another role replaces it.
const session = ref(null);
const loaded = ref(role.value === null);
const screen = ref(startScreen(role.value));
const submitted = ref(null);
const caseRoot = ref(null);
// The application possibilities the law gives this organization. The
// Indienen tab only appears when there is one; this is offering, not
// shielding: the process does not refuse a submission.
const possible = ref([]);
const prefilled = ref({});

function startScreen(r) {
  return startScreenOf(props.process, r);
}

// The route groups of the chosen role.
const may = (routes) => props.process.roles?.[role.value]?.routes?.includes(routes) ?? false;

// The channel of the chosen role, and whether the login has to name the role
// (when more than one role logs in through that channel).
const channelId = computed(() => props.process.roles?.[role.value]?.channel ?? null);
const channel = computed(() => props.process.channels?.[channelId.value] ?? null);
const sendRole = computed(() => roles.value.filter((r) => r.channel === channelId.value).length > 1);

onMounted(async () => {
  if (role.value === null) return;
  api
    .examples()
    .then((e) => (examples.value = e))
    .catch(() => {});
  try {
    const s = await api.session().catch(() => null);
    if (s && props.process.roles?.[s.role]) {
      session.value = s;
      chooseRole(s.role);
    }
  } finally {
    loaded.value = true;
  }
});

function chooseRole(r) {
  role.value = r;
  screen.value = startScreen(r);
  caseRoot.value = null;
  submitted.value = null;
}

const loggedIn = computed(() => session.value !== null && session.value.role === role.value);
const columnsOf = (name) => props.cell.lexostatuses.find((l) => l.name === name)?.columns ?? [];
const worklistColumns = computed(() => columnsOf(props.process.handling?.worklist));
const casesColumns = computed(() => columnsOf(props.process.handling?.cases));

function possibilitiesLoaded(list) {
  possible.value = list.filter((p) => p.verdict === 'possible');
}

// What is fixed beforehand: the field of the chosen window.
function apply(fields) {
  prefilled.value = fields;
  screen.value = 'application';
}

// The gram just submitted, {gram, yaml}: whoever submits sees their own
// submission, not the chronicle.
function onSubmitted(item) {
  submitted.value = item;
  screen.value = 'submitted';
}

async function logout() {
  await api.logout(session.value.channel).catch(() => {});
  session.value = null;
  submitted.value = null;
  possible.value = [];
  prefilled.value = {};
  screen.value = startScreen(role.value);
}

function tab(e) {
  const to = e.detail?.item?.dataset?.screen;
  if (to) {
    screen.value = to;
    caseRoot.value = null;
  }
}

const who = computed(() => sessionText(props.process, session.value));
</script>

<template>
  <template v-if="roles.length > 1">
    <nldd-segmented-control accessible-label="Rol" :value="role" @change="chooseRole($event.detail.value)">
      <nldd-segmented-control-item v-for="r in roles" :key="r.id" :value="r.id" :text="r.label"></nldd-segmented-control-item>
    </nldd-segmented-control>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="loaded && role !== null && !loggedIn && channel">
    <LoginView
      :key="role"
      :channel-id="channelId"
      :channel="channel"
      :role="role"
      :send-role="sendRole"
      @logged-in="session = $event"
    />
  </template>
  <template v-else-if="loaded">
    <nldd-container layout="row" horizontal-alignment="space-between" vertical-alignment="center">
      <nldd-tab-bar size="md" accessible-label="Scherm" @tabchange="tab">
        <nldd-tab-bar-item
          v-if="may('portal') && process.portal"
          data-screen="possibilities"
          text="Wat kan ik aanvragen"
          :current="screen === 'possibilities' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="may('portal') && process.portal && possible.length"
          data-screen="application"
          text="Indienen"
          :current="screen === 'application' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="may('handling') && process.handling"
          data-screen="worklist"
          text="Werkvoorraad"
          :current="screen === 'worklist' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="may('handling') && process.handling?.cases"
          data-screen="cases"
          text="Alle zaken"
          :current="screen === 'cases' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="may('counter') && process.counter"
          data-screen="counter"
          text="Loket"
          :current="screen === 'counter' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="submitted"
          data-screen="submitted"
          text="Ingediend"
          :current="screen === 'submitted' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="may('handling') && process.handling"
          data-screen="chronicle"
          text="Kroniek"
          :current="screen === 'chronicle' || undefined"
        ></nldd-tab-bar-item>
        <nldd-tab-bar-item
          v-if="may('handling') && process.handling && cell.lexostatuses.length"
          data-screen="lexostatus"
          text="Lexostatus"
          :current="screen === 'lexostatus' || undefined"
        ></nldd-tab-bar-item>
      </nldd-tab-bar>
      <nldd-button
        v-if="session"
        variant="neutral-transparent"
        start-icon="logout"
        :text="`Uitloggen (${who})`"
        @click="logout"
      ></nldd-button>
    </nldd-container>
    <nldd-spacer size="24"></nldd-spacer>
    <PossibilitiesView v-if="screen === 'possibilities'" @loaded="possibilitiesLoaded" @apply="apply" />
    <ApplicationView v-else-if="screen === 'application'" :key="JSON.stringify(prefilled)" :prefilled="prefilled" @submitted="onSubmitted" />
    <CounterView v-else-if="screen === 'counter'" @submitted="onSubmitted" />
    <template v-else-if="screen === 'worklist' || screen === 'cases'">
      <CaseView v-if="caseRoot" :key="caseRoot" :root="caseRoot" @back="caseRoot = null" />
      <WorklistView v-else-if="screen === 'worklist'" key="worklist" :columns="worklistColumns" @open="caseRoot = $event" />
      <WorklistView
        v-else
        key="cases"
        source="cases"
        title="Alle zaken"
        subtitle="Elke zaak, ook als er al op is besloten"
        empty="Geen zaken"
        :columns="casesColumns"
        @open="caseRoot = $event"
      />
    </template>
    <template v-else-if="screen === 'submitted' && submitted">
      <nldd-title size="2"><h1>Ingediend</h1></nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <Grams :items="[submitted]" :highlighted="submitted.gram" />
    </template>
    <ChronicleView v-else-if="screen === 'chronicle' && may('handling')" :portal="process.portal" />
    <LexostatusView v-else-if="screen === 'lexostatus' && may('handling')" :lexostatuses="cell.lexostatuses" />
    <nldd-inline-dialog
      v-else
      text="Geen scherm voor deze rol"
      supporting-text="De kroniek en de lexostatussen van een cel zijn niet open; een behandelaar ziet ze in het proces."
    ></nldd-inline-dialog>
  </template>
</template>
