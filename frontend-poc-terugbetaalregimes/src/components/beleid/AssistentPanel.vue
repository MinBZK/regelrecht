<template>
  <div class="assistent">
    <nldd-banner v-if="health === 'onbereikbaar'" variant="accent">
      De assistent-backend draait niet. Start hem met <code>just poc-assistent terugbetaalregimes</code> in een tweede terminal.
    </nldd-banner>
    <nldd-banner v-else-if="health === 'geen-cli'" variant="warning">
      De backend draait, maar de Claude Code CLI is niet gevonden. Installeer die en herstart met <code>just poc-assistent terugbetaalregimes</code>.
    </nldd-banner>

    <!-- Doel staat voor en is de standaard: dat is de vraag waarmee een
         beleidsmaker binnenkomt, en de modus waar het optimalisatiepad
         hieronder voor bestaat. -->
    <nldd-segmented-control size="sm" :value="modus" @change="modus = $event.detail?.value ?? modus">
      <nldd-segmented-control-item value="doel" text="Doel"></nldd-segmented-control-item>
      <nldd-segmented-control-item value="instructie" text="Instructie"></nldd-segmented-control-item>
    </nldd-segmented-control>

    <nldd-form-field
      :label="modus === 'doel' ? 'Doel' : 'Instructie'"
      :supporting-label="modusUitleg"
    >
      <nldd-multi-line-text-field
        :value="prompt"
        :placeholder="placeholder"
        rows="3"
        @input="prompt = $event.detail?.value ?? prompt"
      ></nldd-multi-line-text-field>
    </nldd-form-field>

    <div class="as-knoppen">
      <nldd-button
        :text="streaming ? 'Assistent werkt…' : 'Verstuur'"
        start-icon="send"
        variant="primary"
        :disabled="!beschikbaar || streaming || !prompt.trim()"
        @click="submit"
      ></nldd-button>
      <nldd-button v-if="streaming" text="Stop" start-icon="remove" variant="secondary" @click="stop"></nldd-button>
    </div>

    <!-- Zolang hij bezig is: wat doet hij, hoeveel beurten, hoe lang al. Een
         doel-run kan minuten stil zijn, en zonder dit is dat niet van
         vastgelopen te onderscheiden. -->
    <div v-if="streaming || afronding" class="as-status">
      <nldd-activity-indicator v-if="streaming && !openVraag" size="16" timing="instant"></nldd-activity-indicator>
      <nldd-icon v-else-if="!streaming" name="checked" size="16"></nldd-icon>
      <nldd-icon v-else name="help" size="16"></nldd-icon>
      <span>{{ openVraag ? 'Wacht op jouw keuze.' : statusTekst }}</span>
    </div>

    <!-- De assistent legt een keuze voor en staat stil tot je antwoordt. Dit is
         waar het gesprek zijn waarde krijgt: de beleidsmatige aanname wordt
         gemaakt door de beleidsmaker, niet stilletjes door het model. -->
    <nldd-inline-dialog
      v-if="openVraag"
      variant="alert"
      icon="help"
      :text="openVraag.vraag"
      :supporting-text="openVraag.toelichting ?? ''"
    >
      <div slot="actions" class="as-opties">
        <template v-if="openVraag.meerkeuze">
          <!-- Het gevolg staat in het label zelf: nldd-checkbox-field heeft
               geen supporting-label, en zonder gevolg is het geen keuze die
               een beleidsmaker kan maken. -->
          <nldd-checkbox-field
            v-for="(o, i) in openVraag.opties"
            :key="i"
            :label="o.gevolg ? `${o.label} — ${o.gevolg}` : o.label"
            :checked="aangevinkt.includes(o.label) ? true : undefined"
            @change="vinkAan(o.label, $event)"
          ></nldd-checkbox-field>
          <nldd-button
            size="sm"
            variant="primary"
            text="Doorgeven"
            :disabled="!aangevinkt.length ? true : undefined"
            @click="beantwoord(aangevinkt)"
          ></nldd-button>
        </template>
        <template v-else>
          <nldd-button
            v-for="(o, i) in openVraag.opties"
            :key="i"
            size="sm"
            :variant="i === 0 ? 'primary' : 'secondary'"
            :text="o.gevolg ? `${o.label} — ${o.gevolg}` : o.label"
            @click="beantwoord([o.label])"
          ></nldd-button>
        </template>
      </div>
    </nldd-inline-dialog>

    <!-- Een vervolgbericht, ook terwijl hij nog bezig is: dat komt bij zijn
         volgende beurt binnen. -->
    <div v-if="streaming && !openVraag" class="as-vervolg">
      <nldd-form-field label="Stuur er iets achteraan" supporting-label="komt binnen bij zijn volgende beurt">
        <nldd-text-field
          size="sm"
          :value="vervolg"
          placeholder="Let ook op de kwijtscheldingskosten"
          @input="vervolg = $event.detail?.value ?? vervolg"
        ></nldd-text-field>
      </nldd-form-field>
      <nldd-button
        size="sm"
        variant="secondary"
        start-icon="send"
        text="Insturen"
        :disabled="!vervolg.trim() ? true : undefined"
        @click="stuurVervolg"
      ></nldd-button>
    </div>

    <optimalisatiepad-chart
      v-if="modus === 'doel' && pad.length"
      :pad="pad"
      :reeksen="reeksen"
      :as-titels="['%']"
      :as-formatters="[(v) => `${v}%`]"
      :gekozen="gekozenPunt"
      @kies="kiesPunt"
    />

    <!-- Wat er op de aangeklikte meting stond, met de mogelijkheid die
         tussenstand over te nemen zonder op convergentie te wachten. -->
    <div v-if="gekozenStand" class="as-punt">
      <div class="as-punt-kop">
        <strong>Meting {{ gekozenStand.iteratie }}</strong>
        <span>{{ percent(gekozenStand.pct, 1) }} betalingsproblemen<span v-if="gekozenStand.n">, n={{ gekozenStand.n }}</span></span>
        <nldd-icon-button icon="remove" size="sm" accessible-label="Sluit deze meting" @click="gekozenPunt = null"></nldd-icon-button>
      </div>
      <ul v-if="gekozenStand.wijzigingen?.length" class="as-punt-lijst">
        <li v-for="w in gekozenStand.wijzigingen" :key="`${w.artikel}:${w.naam}`">
          art. {{ w.artikel }} · <code>{{ w.naam }}</code>: {{ w.oud }} → {{ w.nieuw }}
        </li>
      </ul>
      <p v-else class="as-hint">Op deze meting was er nog niets gewijzigd.</p>
      <p v-if="gekozenStand.structuurGewijzigd?.length" class="as-hint">
        Ook de structuur van {{ gekozenStand.structuurGewijzigd.length === 1 ? 'een document' : `${gekozenStand.structuurGewijzigd.length} documenten` }}
        is herschreven; die wijziging staat niet als losse waarden in deze lijst.
      </p>
      <nldd-button
        v-if="gekozenStand.wijzigingen?.length"
        size="sm"
        variant="secondary"
        start-icon="save"
        :text="`Neem deze stand over in ${werkversieLabel}`"
        @click="neemStandOver(gekozenStand)"
      ></nldd-button>
    </div>

    <div v-if="feed.length" ref="feedEl" class="as-feed">
      <div v-for="(item, i) in feed" :key="i" class="as-item" :class="`as-${item.type}`">
        <span v-if="item.type === 'tekst'" class="as-md" v-html="eenvoudigeMarkdown(item.tekst)"></span>
        <template v-else-if="item.type === 'tool'">
          🔧 {{ item.naam }}<span v-if="item.inputText"> {{ item.inputText }}</span>
        </template>
        <template v-else-if="item.type === 'wijziging'">
          ✏️ <strong>{{ item.document_key }}</strong>: {{ item.toelichting }}
        </template>
        <template v-else-if="item.type === 'simulatie'">
          📊 Simulatie ({{ item.doel }}<span v-if="item.n">, n={{ item.n }}</span>):
          {{ item.pct !== null ? percent(item.pct, 1) + ' betalingsproblemen' : 'klaar' }}
        </template>
        <template v-else-if="item.type === 'vraag'">❓ {{ item.vraag }}</template>
        <template v-else-if="item.type === 'gebruiker'">🙋 {{ item.tekst }}</template>
        <template v-else-if="item.type === 'fout'">⚠️ {{ item.melding }}</template>
      </div>
    </div>

    <nldd-banner v-if="overlays && !Object.keys(overlays).length" variant="accent">
      De assistent heeft niets gewijzigd.
    </nldd-banner>
    <nldd-button
      v-if="overlays && Object.keys(overlays).length"
      :text="`Overnemen in ${werkversieLabel}`"
      start-icon="save"
      variant="secondary"
      @click="takeOverlays"
    ></nldd-button>
    <p v-if="overlays && Object.keys(overlays).length" class="as-hint">
      Zet de wijziging van de assistent als bewerking in de werkversie ({{ Object.keys(overlays).length }}
      {{ Object.keys(overlays).length === 1 ? 'document' : 'documenten' }}); zichtbaar via Bekijk wijzigingen, terug te
      draaien met Terugzetten, te bewaren met Bewaar als variant.
    </p>
  </div>
</template>

<script setup>
import { ref, computed, nextTick, onMounted, onUnmounted } from 'vue';
import { useAssistent } from '../../composables/useAssistent.js';
import { useLawStore } from '../../engine/lawStore.js';
import { percent } from '../../lib/format.js';
import { resolveToken } from '../../lib/tokens.js';
import { b } from '../../basePad.js';
import OptimalisatiepadChart from '@regelrecht/frontend-shared/components/OptimalisatiepadChart.vue';

// Geen `metrics`-emit meer. De assistent meet met zijn eigen n op zijn eigen
// tussenstand; die cijfers naast de tegels zetten zou de doorrekening van de
// gebruiker stil overschrijven met een tussenmeting. Ze horen thuis in de
// feed en in het optimalisatiepad hieronder, en nergens anders.
const { streaming, run, stuur, antwoord, abort } = useAssistent();
const {
  applyOverlays, applyDefinitionChange, docPathVoorKey,
  lawDocsFor, werkversie, werkversieLabel,
} = useLawStore();

/** Minimale markdown voor de assistenttekst: vet, code, regeleinden; de rest blijft tekst. */
function eenvoudigeMarkdown(tekst) {
  const esc = String(tekst ?? '').replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
  return esc
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\n/g, '<br>');
}

function stop() {
  abort();
  feed.value.push({ type: 'tekst', tekst: 'Afgebroken.' });
}

// 'onbekend' (nog niet gepeild) | 'onbereikbaar' | 'geen-cli' | 'ok'
const health = ref('onbekend');
const beschikbaar = computed(() => health.value === 'ok');
let healthTimer = null;

async function peilHealth() {
  try {
    const res = await fetch(b('/api/health'));
    if (!res.ok) throw new Error();
    const body = await res.json();
    health.value = body.cli ? 'ok' : 'geen-cli';
  } catch {
    health.value = 'onbereikbaar';
  }
  // Blijf peilen als de backend (nog) niet bereikbaar is, zodat "just server"
  // starten in een tweede terminal het paneel vanzelf ontgrendelt.
  if (health.value !== 'ok' && !healthTimer) {
    healthTimer = setInterval(async () => {
      await peilHealth();
      if (health.value === 'ok') {
        clearInterval(healthTimer);
        healthTimer = null;
      }
    }, 5000);
  }
}

const modus = ref('doel');
const prompt = ref('');
const feed = ref([]);
const pad = ref([]); // [{iteratie, pct}] voor de doel-modus
const overlays = ref(null);
const feedEl = ref(null);

const placeholder = computed(() =>
  modus.value === 'doel'
    ? 'Minimaliseer het aantal debiteuren met betalingsproblemen zonder de kwijtscheldingskosten meer dan te verdubbelen'
    : 'Verhoog de draagkrachtvrije voet van SF15-oud naar 84% van het belastbaar minimumloon',
);

/** Eén reeks: het aandeel debiteuren met betalingsproblemen, in procenten. */
const reeksen = [{
  sleutel: 'pct',
  label: 'betalingsproblemen',
  kleur: resolveToken('--semantics-content-accent-color', '#154273'),
  formatter: (v) => `${v}%`,
}];

/** Laatste voortgangsmelding van de backend, en de afronding na afloop. */
const voortgang = ref(null);
const afronding = ref(null);

/** De keuze die nu voorligt, als de assistent er een stelde. */
const openVraag = ref(null);
const aangevinkt = ref([]);
const vervolg = ref('');

function vinkAan(label, ev) {
  const aan = ev?.detail?.checked ?? ev?.target?.checked;
  aangevinkt.value = aan
    ? [...new Set([...aangevinkt.value, label])]
    : aangevinkt.value.filter((l) => l !== label);
}

async function beantwoord(keuzes) {
  const vraag = openVraag.value;
  if (!vraag) return;
  openVraag.value = null;
  aangevinkt.value = [];
  try {
    await antwoord(vraag.id, keuzes);
  } catch (e) {
    // Valt de verbinding weg terwijl de vraag openstaat, dan is het gesprek
    // voorbij en heeft terugzetten van de keuze geen zin: er is niemand meer
    // om hem aan te geven. Zeg dat, in plaats van een knop te tonen die het
    // opnieuw niet doet.
    const weg = !streaming.value;
    feed.value.push({
      type: 'fout',
      melding: weg
        ? 'De verbinding met de assistent is weggevallen, dus je keuze kon niet meer aankomen. Stel de vraag opnieuw.'
        : `Antwoord doorgeven mislukt: ${e?.message ?? e}`,
    });
    if (!weg) openVraag.value = vraag;
  }
}

async function stuurVervolg() {
  const tekst = vervolg.value.trim();
  if (!tekst) return;
  vervolg.value = '';
  try {
    await stuur(tekst);
  } catch (e) {
    feed.value.push({ type: 'fout', melding: `Insturen mislukt: ${e?.message ?? e}` });
    vervolg.value = tekst;
  }
}

/** Namen van de tools zoals een beleidsmaker ze zou noemen. */
const TOOLTEKST = {
  lees_regelgeving: 'leest de regelgeving',
  wijzig_definitie: 'past een waarde aan',
  wijzig_regelgeving: 'herschrijft een artikel',
  lees_handelingen: 'leest het uitvoeringslastmodel',
  wijzig_handelingen: 'past het uitvoeringslastmodel aan',
  simuleer_personas: "rekent de persona's door",
  simuleer_populatie: 'rekent de populatie door',
};

const statusTekst = computed(() => {
  if (afronding.value) {
    const { beurten, seconden } = afronding.value;
    return `Klaar in ${duur(seconden)}, ${beurten} ${beurten === 1 ? 'beurt' : 'beurten'}.`;
  }
  const v = voortgang.value;
  if (!v) return 'Bezig…';
  const wat = v.tool ? (TOOLTEKST[v.tool] ?? v.tool)
    : v.status === 'requesting' ? 'denkt na'
      : 'bezig';
  const beurt = v.beurten ? `, beurt ${v.beurten}` : '';
  return `${wat[0].toUpperCase()}${wat.slice(1)}${beurt} · ${duur(v.seconden)}`;
});

function duur(seconden) {
  const s = Number(seconden) || 0;
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  const rest = s % 60;
  return rest ? `${m} min ${rest} s` : `${m} min`;
}

/** Index van het aangeklikte punt in `pad`, of null. */
const gekozenPunt = ref(null);
const gekozenStand = computed(() => (gekozenPunt.value === null ? null : pad.value[gekozenPunt.value] ?? null));

function kiesPunt(index) {
  gekozenPunt.value = gekozenPunt.value === index ? null : index;
}

/**
 * Neem de stand van één meting over in de werkversie. Dat is niet de eindstand
 * van de assistent maar een tussenstap, en dat is precies waar dit voor is: een
 * pad loopt soms door een uitkomst heen die je beter bevalt dan waar hij
 * uitkomt.
 */
function neemStandOver(stand) {
  const mislukt = [];
  for (const w of stand.wijzigingen) {
    // De sleutel van de assistent is "id@valid_from"; de store vindt daar zijn
    // documentpad bij.
    const pad = docPathVoorKey(w.document_key);
    if (!pad) { mislukt.push(`${w.naam} (document ${w.document_key} niet gevonden)`); continue; }
    try {
      applyDefinitionChange(pad, w.artikel, w.naam, w.nieuw);
    } catch (e) {
      mislukt.push(`${w.naam} (${e?.message ?? e})`);
    }
  }
  const aantal = stand.wijzigingen.length - mislukt.length;
  feed.value.push({
    type: mislukt.length ? 'fout' : 'tekst',
    tekst: `Meting ${stand.iteratie} overgenomen in ${werkversieLabel.value}: ${aantal} van de ${stand.wijzigingen.length} wijzigingen.`,
    melding: mislukt.length ? `Niet overgenomen: ${mislukt.join('; ')}.` : undefined,
  });
  gekozenPunt.value = null;
}

/**
 * Wat de twee modi van elkaar onderscheidt, in één regel. Zonder dit is het
 * verschil alleen uit het gedrag af te leiden: doel rekent iteratief naar een
 * uitkomst toe en mag daar zestig beurten over doen, instructie voert één
 * wijziging uit en laat het effect zien.
 */
const modusUitleg = computed(() =>
  modus.value === 'doel'
    ? 'Je weet wat je wilt bereiken. De assistent probeert wijzigingen, meet het effect en stelt bij tot het niet verder verbetert.'
    : 'Je weet wat je wilt veranderen. De assistent voert die ene wijziging uit en rekent door.',
);

onMounted(peilHealth);
onUnmounted(() => {
  if (healthTimer) clearInterval(healthTimer);
});

async function submit() {
  feed.value = [];
  pad.value = [];
  gekozenPunt.value = null;
  overlays.value = null;
  voortgang.value = null;
  afronding.value = null;
  openVraag.value = null;
  aangevinkt.value = [];
  vervolg.value = '';
  let iteratie = 0;

  // De assistent werkt op de werkversie: stuur die documenten mee als beginstand.
  const documenten = (await lawDocsFor(werkversie.value)).map((d) => ({ key: `${d.entry.id}@${d.entry.valid_from ?? ''}`, yaml: d.yaml }));
  feed.value.push({ type: 'tekst', tekst: `Werkt op ${werkversieLabel.value}.` });
  await run({ modus: modus.value, prompt: prompt.value, documenten }, (ev) => {
    if (ev.type === 'voortgang') {
      voortgang.value = ev;
    } else if (ev.type === 'vraag') {
      openVraag.value = ev;
      aangevinkt.value = [];
      feed.value.push({ type: 'vraag', vraag: ev.vraag });
    } else if (ev.type === 'vraag_verlopen') {
      // Niemand antwoordde; de assistent kiest zelf verder en zegt dat erbij.
      if (openVraag.value?.id === ev.id) openVraag.value = null;
      feed.value.push({ type: 'tekst', tekst: 'Geen antwoord gegeven; de assistent kiest zelf verder.' });
    } else if (ev.type === 'gebruiker') {
      feed.value.push({ type: 'gebruiker', tekst: ev.tekst });
    } else if (ev.type === 'gesprek') {
      // alleen het id; useAssistent houdt het bij
    } else if (ev.type === 'tekst_deel') {
      // Tekst terwijl hij getypt wordt. De losse stukjes gaan in één regel in
      // de feed; het complete `tekst`-event erna vervangt die regel, zodat er
      // niet twee keer hetzelfde komt te staan.
      const laatste = feed.value[feed.value.length - 1];
      if (laatste?.type === 'tekst' && laatste.deels) laatste.tekst += ev.tekst;
      else feed.value.push({ type: 'tekst', tekst: ev.tekst, deels: true });
    } else if (ev.type === 'tekst') {
      const laatste = feed.value[feed.value.length - 1];
      if (laatste?.deels) feed.value[feed.value.length - 1] = { type: 'tekst', tekst: ev.tekst };
      else feed.value.push(ev);
    } else if (ev.type === 'tool') {
      const inputText = ev.input
        ? Object.entries(ev.input).map(([k, v]) => `${k}=${v}`).join(' ')
        : '';
      feed.value.push({ type: 'tool', naam: ev.naam, inputText });
    } else if (ev.type === 'simulatie') {
      const pct = ev.metrics?.pctBetalingsprobleem ?? null;
      feed.value.push({ type: 'simulatie', doel: ev.doel, n: ev.n, pct });
      if (ev.doel === 'populatie' && pct !== null) {
        pad.value.push({
          iteratie: ++iteratie,
          waarden: { pct: Math.round(pct * 1000) / 10 },
          // De stand waarop deze meting rust, zodat het punt aanklikbaar is.
          pct,
          n: ev.n ?? null,
          wijzigingen: ev.wijzigingen ?? [],
          structuurGewijzigd: ev.structuurGewijzigd ?? [],
        });
      }
    } else if (ev.type === 'klaar') {
      overlays.value = ev.overlays ?? null;
      afronding.value = { beurten: ev.beurten ?? 0, seconden: ev.seconden ?? 0 };
      voortgang.value = null;
      openVraag.value = null;
    } else {
      feed.value.push(ev);
    }
    nextTick(() => {
      if (feedEl.value) feedEl.value.scrollTop = feedEl.value.scrollHeight;
    });
  });
  // De stream is dicht. Staat er nog een vraag open, dan komt er niemand meer
  // om hem te beantwoorden; een dialoog laten staan die niets meer doet is
  // erger dan hem weghalen.
  if (openVraag.value) {
    openVraag.value = null;
    feed.value.push({ type: 'tekst', tekst: 'Het gesprek is afgelopen terwijl er een keuze openstond.' });
  }
}

function takeOverlays() {
  if (!overlays.value) return;
  applyOverlays(overlays.value);
  feed.value.push({ type: 'tekst', tekst: `Overgenomen in ${werkversieLabel.value}.` });
  overlays.value = null;
}
</script>

<style scoped>
.assistent { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.as-feed {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 280px;
  overflow-y: auto;
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  border: 1px solid var(--semantics-dividers-color);
}
.as-item { font-size: 0.9em; line-height: 1.45; }
.as-tekst { color: var(--semantics-content-color); }
.as-tool { color: var(--semantics-content-secondary-color); font-family: var(--primitives-font-family-monospace, monospace); font-size: 0.85em; }
.as-wijziging { color: var(--semantics-content-accent-color); }
.as-simulatie { color: var(--semantics-content-secondary-color); }
.as-fout { color: var(--semantics-content-critical-color); }
.as-vraag { color: var(--semantics-content-accent-color); font-weight: 600; }
.as-gebruiker { color: var(--semantics-content-color); font-weight: 600; }
.as-opties { display: flex; flex-direction: column; gap: var(--primitives-space-8); align-items: flex-start; }
.as-vervolg { display: flex; gap: var(--primitives-space-8); align-items: flex-end; }
.as-vervolg nldd-form-field { flex: 1; }
.as-knoppen { display: flex; gap: var(--primitives-space-8); align-items: center; }
.as-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.as-status {
  display: flex; align-items: center; gap: var(--primitives-space-8);
  font-size: 0.85em; color: var(--semantics-content-secondary-color);
  font-variant-numeric: tabular-nums;
}
.as-punt {
  display: flex; flex-direction: column; gap: var(--primitives-space-8);
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  border: 1px solid var(--semantics-dividers-color);
}
.as-punt-kop { display: flex; align-items: center; gap: var(--primitives-space-8); font-size: 0.9em; }
.as-punt-kop nldd-icon-button { margin-left: auto; }
.as-punt-lijst { margin: 0; padding-left: 1.2em; font-size: 0.85em; display: flex; flex-direction: column; gap: 2px; }
.as-md code { font-size: 0.9em; }
</style>
