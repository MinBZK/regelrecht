/**
 * De Nederlandse teksten van de demo. Dit is de bron.
 *
 * Flat keys, dotted and namespaced by the screen they belong to. Flat rather
 * than nested on purpose: the parity check is then a set difference over
 * `Object.keys()`, and a key that shows up on screen can be grepped for
 * literally.
 *
 * Edit this file and `en.js` in the same change. After changing a Dutch string
 * whose translation still holds, run `node scripts/i18n-bless.mjs` to record
 * the new source hash; otherwise the staleness test will (correctly) flag it.
 */
export default {
  // ---- werkbalk: tabbladen -------------------------------------------------
  'app.tabs.home': 'Home',
  'app.tabs.presentatie': 'Presentatie',
  'app.tabs.wetten': 'Wetten',
  'app.tabs.graaf': 'Graaf',
  'app.tabs.scenarios': "Scenario's",
  'app.tabs.simulatie': 'Simulatie',
  'app.tabs.portaal': 'Mijn overheid',
  'app.tabs.zaaksysteem': 'Zaaksysteem',
  'app.tabs.label': 'Demo-onderdeel',
  'app.tabs.goto': 'Ga naar',

  // ---- werkbalk: knoppen en menu's ----------------------------------------
  'app.cases.pending.one': '{n} te beoordelen',
  'app.cases.pending.other': '{n} te beoordelen',
  'app.delegation.label': 'Namens wie',
  'app.delegation.self': 'Mezelf',
  'app.profile.label': 'Demoprofiel',

  'app.language.label': 'Taal',
  'app.language.nl': 'Nederlands',
  'app.language.en': 'English',

  'app.features.label': 'Features',
  'app.features.DELEGATION': 'Machtigingen',
  'app.features.CHANGE_WIZARD': 'Wijziging doorgeven',
  'app.features.HARMONIZE': 'Harmonisatie',
  'app.features.AUTO_APPROVE_CLAIMS': 'Correcties direct goedkeuren',
  'app.features.manualReview': 'Alle aanvragen handmatig beoordelen',
  'app.features.reset': 'Terug naar het profiel',

  'app.appearance.label': 'Weergave',
  'app.appearance.auto': 'Systeem',
  'app.appearance.light': 'Licht',
  'app.appearance.dark': 'Donker',

  'app.demo.label': 'Demo',
  'app.demo.fullscreen': 'Volledig scherm',
  'app.demo.reset': 'Demo resetten…',

  // ---- laden, fouten, resetten --------------------------------------------
  'app.loading': 'Wetten en engine laden…',
  'app.error.title': 'De demo kon niet starten',
  'app.reset.title': 'Demo resetten?',
  'app.reset.body': "Alle aanvragen en correcties uit deze demo worden gewist. De wetten en persona's blijven.",
  'app.reset.label': 'Demo resetten',
  'app.reset.confirm': 'Resetten',
  'app.reset.cancel': 'Annuleren',

  // ---- waarden opmaken -----------------------------------------------------
  // Twee soorten "niets" (RFC-036): `null` is een afwezigheid die de gegevens
  // zelf stellen, Unknown is een feit dat niemand heeft aangeleverd. Die twee
  // krijgen nooit hetzelfde woord.
  'format.unknown': 'onbekend',
  'format.none': 'geen',
  'format.yes': 'Ja',
  'format.no': 'Nee',
  'format.years': '{n} jaar',
  'format.items.one': '{n} item',
  'format.items.other': '{n} items',
  'format.missing': 'ontbreekt: {facts}',
};
