/**
 * The English translation of the demo's interface. Dutch (`nl.js`) is the source.
 *
 * Register: British-leaning government English, plain and unadorned, the same
 * voice the docs landing page uses. Not marketing copy — this describes a tool
 * a civil servant operates.
 *
 * Tab names are the exception to "translate everything": `Graaf` becomes
 * `Graph` and `Wetten` becomes `Laws`, but `Mijn overheid` becomes
 * `My government` rather than a literal "My authority", because it names the
 * citizen-facing portal the Dutch government actually runs.
 *
 * After changing a Dutch original, retranslate here and run
 * `node scripts/i18n-bless.mjs`.
 */
export default {
  // ---- toolbar: tabs -------------------------------------------------------
  'app.tabs.home': 'Home',
  'app.tabs.presentatie': 'Presentation',
  'app.tabs.wetten': 'Laws',
  'app.tabs.graaf': 'Graph',
  'app.tabs.scenarios': 'Scenarios',
  'app.tabs.simulatie': 'Simulation',
  'app.tabs.portaal': 'My government',
  'app.tabs.zaaksysteem': 'Case system',
  'app.tabs.label': 'Demo section',
  'app.tabs.goto': 'Go to',

  // ---- toolbar: buttons and menus -----------------------------------------
  'app.cases.pending.one': '{n} to review',
  'app.cases.pending.other': '{n} to review',
  'app.delegation.label': 'Acting for',
  'app.delegation.self': 'Myself',
  'app.profile.label': 'Demo profile',

  'app.language.label': 'Language',
  'app.language.nl': 'Nederlands',
  'app.language.en': 'English',

  'app.features.label': 'Features',
  'app.features.DELEGATION': 'Authorisations',
  'app.features.CHANGE_WIZARD': 'Report a change',
  'app.features.HARMONIZE': 'Harmonisation',
  'app.features.AUTO_APPROVE_CLAIMS': 'Approve corrections immediately',
  'app.features.manualReview': 'Review every application by hand',
  'app.features.reset': 'Back to the profile',

  'app.appearance.label': 'Appearance',
  'app.appearance.auto': 'System',
  'app.appearance.light': 'Light',
  'app.appearance.dark': 'Dark',

  'app.demo.label': 'Demo',
  'app.demo.fullscreen': 'Full screen',
  'app.demo.reset': 'Reset the demo…',

  // ---- loading, errors, resetting -----------------------------------------
  'app.loading': 'Loading laws and engine…',
  'app.error.title': 'The demo could not start',
  'app.reset.title': 'Reset the demo?',
  'app.reset.body': 'Every application and correction made in this demo is erased. The laws and personas stay.',
  'app.reset.label': 'Reset the demo',
  'app.reset.confirm': 'Reset',
  'app.reset.cancel': 'Cancel',

  // ---- formatting values ---------------------------------------------------
  // Two kinds of "nothing" (RFC-036): `null` is an absence the data states,
  // Unknown is a fact nobody supplied. Never the same word.
  'format.unknown': 'unknown',
  'format.none': 'none',
  'format.yes': 'Yes',
  'format.no': 'No',
  'format.years': '{n} years',
  'format.items.one': '{n} item',
  'format.items.other': '{n} items',
  'format.missing': 'missing: {facts}',
};
