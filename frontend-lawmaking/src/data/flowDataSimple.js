/**
 * Flow data model: maps Git/CI/CD concepts to Dutch law-making process.
 *
 * Layout uses a grid coordinate system:
 *   - col 0: main branch (Corpus Juris)
 *   - col 1: feature branch (Wetsvoorstel)
 *   - col 2: CI checks (off to the side)
 *
 * Each row is one step in the vertical flow.
 */

export const branches = [
  {
    id: 'main',
    label: 'Corpus Juris',
    gitLabel: 'main',
    color: 'var(--color-branch-main)',
    col: 0,
    startRow: 0,
    endRow: 12,
  },
  {
    id: 'feature',
    label: 'Wetsvoorstel',
    gitLabel: 'feature/*',
    color: 'var(--color-branch-feature)',
    col: 1,
    startRow: 1,
    endRow: 9,
  },
];

export const stages = [
  {
    id: 'corpus-start',
    branch: 'main',
    type: 'commit',
    gitLabel: 'main branch',
    lawLabel: 'Corpus Juris',
    subtitle: 'Geldend recht',
    description:
      'Het geheel van alle geldende Nederlandse wetgeving — de "main branch". ' +
      'Net als in Git is dit de enige bron van waarheid: wat hier staat, is geldend recht.',
    col: 0,
    row: 0,
    step: 0,
  },
  {
    id: 'initiatief',
    branch: 'feature',
    type: 'branch',
    gitLabel: 'git checkout -b',
    lawLabel: 'Initiatief / Wetsvoorstel',
    subtitle: 'Nieuw voorstel',
    description:
      'Een wetgevend initiatief begint als een branch vanuit het Corpus Juris. ' +
      'Een ministerie of Kamerlid "forkt" de bestaande wet en begint met wijzigen — ' +
      'precies zoals een developer een feature branch aanmaakt.',
    col: 1,
    row: 1,
    step: 1,
  },
  {
    id: 'concept',
    branch: 'feature',
    type: 'commit',
    gitLabel: 'commit',
    lawLabel: 'Conceptwetsvoorstel',
    subtitle: 'Eerste versie',
    description:
      'Het eerste concept van de wet wordt geschreven — de eerste commit op de branch. ' +
      'Net als in software wordt er geïtereerd: de tekst wordt herzien, aangescherpt, ' +
      'en opnieuw gecommit.',
    col: 1,
    row: 2,
    step: 2,
  },
  {
    id: 'raad-van-state',
    branch: 'ci',
    type: 'ci-check',
    gitLabel: 'CI check',
    lawLabel: 'Raad van State advies',
    subtitle: 'Juridische toets',
    description:
      'De Raad van State toetst het voorstel aan de Grondwet en bestaande wetgeving. ' +
      'Dit is vergelijkbaar met een CI check: een geautomatiseerde (in dit geval: ' +
      'institutionele) kwaliteitscontrole voordat het voorstel verder gaat.',
    col: 2,
    row: 3,
    step: 3,
  },
  {
    id: 'uitvoeringstoets',
    branch: 'ci',
    type: 'ci-check',
    gitLabel: 'CI check',
    lawLabel: 'Uitvoeringstoets',
    subtitle: 'Uitvoerbaarheid',
    description:
      'Uitvoeringsorganisaties (UWV, Belastingdienst, SVB) toetsen of de wet ' +
      'uitvoerbaar is — vergelijkbaar met integratietesten die controleren of de ' +
      'code daadwerkelijk werkt in productie.',
    col: 2,
    row: 4,
    step: 4,
  },
  {
    id: 'amendement',
    branch: 'feature',
    type: 'commit',
    gitLabel: 'commit (fix)',
    lawLabel: 'Verwerking advies',
    subtitle: 'Aanpassingen',
    description:
      'Op basis van de adviezen wordt het voorstel aangepast — nieuwe commits ' +
      'op de branch. Net als bij code review feedback: je verwerkt het commentaar ' +
      'en pushed opnieuw.',
    col: 1,
    row: 5,
    step: 5,
  },
  {
    id: 'tweede-kamer',
    branch: 'feature',
    type: 'review',
    gitLabel: 'Pull Request review',
    lawLabel: 'Tweede Kamer',
    subtitle: 'Behandeling & stemming',
    description:
      'De Tweede Kamer behandelt het voorstel — vergelijkbaar met een Pull Request ' +
      'review. Kamerleden dienen amendementen in (review comments), debatteren ' +
      '(discussion), en stemmen uiteindelijk (approve/reject).',
    col: 1,
    row: 6,
    step: 6,
  },
  {
    id: 'kamer-amendement',
    branch: 'feature',
    type: 'commit',
    gitLabel: 'commit (amendment)',
    lawLabel: 'Amendementen',
    subtitle: 'Wijzigingen door Kamer',
    description:
      'Amendementen aangenomen door de Tweede Kamer worden doorgevoerd — ' +
      'extra commits naar aanleiding van code review. De branch evolueert ' +
      'mee met de feedback.',
    col: 1,
    row: 7,
    step: 7,
  },
  {
    id: 'eerste-kamer',
    branch: 'feature',
    type: 'review',
    gitLabel: 'Second reviewer',
    lawLabel: 'Eerste Kamer',
    subtitle: 'Laatste controle',
    description:
      'De Eerste Kamer is de tweede reviewer — een chambre de réflexion. ' +
      'Ze kunnen het voorstel alleen aannemen of verwerpen (geen amendementen), ' +
      'vergelijkbaar met een senior reviewer die approve of reject geeft.',
    col: 1,
    row: 8,
    step: 8,
  },
  {
    id: 'koninklijk-besluit',
    branch: 'main',
    type: 'merge',
    gitLabel: 'Merge commit',
    lawLabel: 'Koninklijk Besluit',
    subtitle: 'Bekrachtiging',
    description:
      'De Koning ondertekent het wetsvoorstel — dit is de merge commit. ' +
      'De feature branch wordt samengevoegd met main. Het Koninklijk Besluit ' +
      'is het formele moment waarop de wet onderdeel wordt van het Corpus Juris.',
    col: 0,
    row: 9,
    step: 9,
  },
  {
    id: 'staatsblad',
    branch: 'main',
    type: 'deploy',
    gitLabel: 'CD / Deploy',
    lawLabel: 'Publicatie Staatsblad',
    subtitle: 'Bekendmaking',
    description:
      'Publicatie in het Staatsblad is de deployment: de wet wordt beschikbaar ' +
      'gemaakt voor iedereen. Vergelijkbaar met een deploy naar productie — ' +
      'de code is gemerged en nu live.',
    col: 0,
    row: 10,
    step: 10,
  },
  {
    id: 'inwerkingtreding',
    branch: 'main',
    type: 'release',
    gitLabel: 'Release / Go-live',
    lawLabel: 'Inwerkingtreding',
    subtitle: 'De wet geldt',
    description:
      'De wet treedt in werking — de release date. Soms valt dit samen met ' +
      'publicatie, soms is er een overgangsperiode (vergelijkbaar met een ' +
      'feature flag of gefaseerde rollout).',
    col: 0,
    row: 11,
    step: 11,
  },
  {
    id: 'corpus-updated',
    branch: 'main',
    type: 'commit',
    gitLabel: 'HEAD',
    lawLabel: 'Corpus Juris',
    subtitle: 'Bijgewerkt',
    description:
      'Het Corpus Juris is bijgewerkt met de nieuwe wet. De main branch ' +
      'gaat door — klaar voor het volgende wetsvoorstel.',
    col: 0,
    row: 12,
    step: 12,
  },
];

export const connections = [
  // Branch off from main to feature
  { from: 'corpus-start', to: 'initiatief', type: 'branch-off' },
  // Feature branch commits flow down
  { from: 'initiatief', to: 'concept', type: 'straight' },
  // CI checks fork off to the right
  { from: 'concept', to: 'raad-van-state', type: 'ci-fork' },
  { from: 'raad-van-state', to: 'uitvoeringstoets', type: 'ci-chain' },
  // CI returns to feature branch
  { from: 'uitvoeringstoets', to: 'amendement', type: 'ci-return' },
  // Continue on feature branch
  { from: 'amendement', to: 'tweede-kamer', type: 'straight' },
  { from: 'tweede-kamer', to: 'kamer-amendement', type: 'straight' },
  { from: 'kamer-amendement', to: 'eerste-kamer', type: 'straight' },
  // Merge back into main
  { from: 'eerste-kamer', to: 'koninklijk-besluit', type: 'merge-in' },
  // Main branch continues
  { from: 'koninklijk-besluit', to: 'staatsblad', type: 'straight' },
  { from: 'staatsblad', to: 'inwerkingtreding', type: 'straight' },
  { from: 'inwerkingtreding', to: 'corpus-updated', type: 'straight' },
  // Main branch also continues past the branch-off point
  { from: 'corpus-start', to: 'koninklijk-besluit', type: 'main-continues' },
];
