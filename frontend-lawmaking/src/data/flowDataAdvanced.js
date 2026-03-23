/**
 * Advanced flow data: the full Dutch legislative process.
 *
 * Layout (7 columns, ~45 rows):
 *   col 0: main (Corpus Juris)
 *   col 1: ministry internal / pre-parliamentary preparation
 *   col 2: formal bill through parliament (Tweede Kamer / Eerste Kamer)
 *   col 3: CI checks / external reviews / advisory bodies
 *   col 4: amendment branches
 *   col 5: sub-amendments, concurrent amendment, initiatiefwet
 *   col 6: delegated legislation (AMvB), novelle loop-back
 */

export const branches = [
  {
    id: 'main',
    label: 'Corpus Juris',
    gitLabel: 'main',
    color: 'var(--color-branch-main)',
    col: 0,
    startRow: 0,
    endRow: 46,
  },
  {
    id: 'ministry',
    label: 'Ministerie',
    gitLabel: 'feature/voorbereiding',
    color: 'var(--color-branch-feature)',
    col: 1,
    startRow: 1,
    endRow: 18,
  },
  {
    id: 'policy',
    label: 'Beleidsteam',
    gitLabel: 'feature/beleid',
    color: 'var(--color-branch-feature)',
    col: 2,
    startRow: 2,
    endRow: 4,
  },
  {
    id: 'parliament',
    label: 'Parlement',
    gitLabel: 'feature/wetsvoorstel',
    color: 'var(--color-branch-feature)',
    col: 2,
    startRow: 19,
    endRow: 40,
  },
  {
    id: 'amendments',
    label: 'Amendementen',
    gitLabel: 'patches/*',
    color: 'var(--color-review)',
    col: 4,
    startRow: 28,
    endRow: 32,
  },
  {
    id: 'sub-amendments',
    label: 'Subamendementen',
    gitLabel: 'patches/sub/*',
    color: 'var(--color-review)',
    col: 5,
    startRow: 28,
    endRow: 32,
  },
  {
    id: 'initiatief',
    label: 'Initiatiefwet',
    gitLabel: 'external-contributor/*',
    color: 'var(--color-release)',
    col: 5,
    startRow: 19,
    endRow: 22,
  },
  {
    id: 'delegated',
    label: 'Gedelegeerde wetgeving',
    gitLabel: 'config/amvb',
    color: 'var(--color-deploy)',
    col: 6,
    startRow: 38,
    endRow: 43,
  },
];

export const phases = [
  { id: 'A', label: 'A. Departementale voorbereiding', startRow: 1, endRow: 4, color: 'var(--color-branch-feature)' },
  { id: 'B', label: 'B. Interdepartementaal', startRow: 5, endRow: 9, color: 'var(--color-ci)' },
  { id: 'C', label: 'C. Externe consultatie', startRow: 10, endRow: 12, color: 'var(--color-review)' },
  { id: 'D', label: 'D. Kabinet', startRow: 13, endRow: 14, color: 'var(--color-branch-main)' },
  { id: 'E', label: 'E. Raad van State', startRow: 15, endRow: 18, color: 'var(--color-ci)' },
  { id: 'F', label: 'F. Tweede Kamer', startRow: 19, endRow: 34, color: 'var(--color-review)' },
  { id: 'G', label: 'G. Eerste Kamer', startRow: 35, endRow: 38, color: 'var(--color-review)' },
  { id: 'H', label: 'H. Bekrachtiging & publicatie', startRow: 39, endRow: 46, color: 'var(--color-deploy)' },
];

export const stages = [
  // === Main branch start ===
  {
    id: 'corpus-start',
    branch: 'main',
    type: 'commit',
    gitLabel: 'main branch',
    lawLabel: 'Corpus Juris',
    subtitle: 'Geldend recht',
    description:
      'Het geheel van alle geldende Nederlandse wetgeving — de "main branch". ' +
      'Wat hier staat, is geldend recht.',
    col: 0, row: 0, step: 0,
  },

  // === Phase A: Ministry Internal (parallel tracks) ===
  {
    id: 'beleidsidee',
    branch: 'ministry',
    type: 'branch',
    gitLabel: 'git checkout -b',
    lawLabel: 'Beleidsidee',
    subtitle: 'Aanleiding',
    description:
      'Een wetgevend traject begint met een aanleiding: het regeerakkoord, een EU-richtlijn, ' +
      'een rechterlijke uitspraak, of een maatschappelijk probleem.',
    col: 1, row: 1, step: 1,
  },
  {
    id: 'beleidsnota',
    branch: 'policy',
    type: 'branch',
    gitLabel: 'git checkout -b (parallel)',
    lawLabel: 'Beleidsnota',
    subtitle: 'Beleidsmedewerker schrijft',
    description:
      'Beleidsmedewerkers schrijven een beleidsnota met de gewenste maatschappelijke effecten. ' +
      'Dit is een parallelle branch: beleid en recht worden gelijktijdig ontwikkeld.',
    col: 2, row: 2, step: 2,
  },
  {
    id: 'conceptwet',
    branch: 'ministry',
    type: 'commit',
    gitLabel: 'commit',
    lawLabel: 'Conceptwetsvoorstel',
    subtitle: 'Wetgevingsjurist schrijft',
    description:
      'De wetgevingsjurist vertaalt het beleid naar juridische tekst: het wetsvoorstel ' +
      'én de memorie van toelichting. Meerdere interne iteraties volgen.',
    col: 1, row: 3, step: 3,
  },
  {
    id: 'mvt',
    branch: 'policy',
    type: 'commit',
    gitLabel: 'commit',
    lawLabel: 'Memorie van Toelichting',
    subtitle: 'Onderbouwing & artikelsgewijze toelichting',
    description:
      'De MvT beschrijft het doel, de achtergrond, en de artikelsgewijze toelichting. ' +
      'Dit is een apart document dat meeloopt met het wetsvoorstel — als een parallelle branch ' +
      'die uiteindelijk samenkomt.',
    col: 2, row: 3, step: 3,
  },
  {
    id: 'merge-beleid',
    branch: 'ministry',
    type: 'merge',
    gitLabel: 'merge (internal)',
    lawLabel: 'Samenvoeging',
    subtitle: 'Beleid + recht samengebracht',
    description:
      'De beleidsnota en het juridische concept worden samengevoegd tot één coherent pakket: ' +
      'wetsvoorstel + MvT. Vergelijkbaar met het mergen van twee feature branches.',
    col: 1, row: 4, step: 4,
  },
  {
    id: 'dept-toets',
    branch: 'ministry',
    type: 'ci-check',
    gitLabel: 'lint / internal CI',
    lawLabel: 'Departementale toets',
    subtitle: 'Interne kwaliteitscontrole',
    description:
      'De directie Wetgeving/JZ toetst aan de Aanwijzingen voor de regelgeving. ' +
      'Bij JenV ook een wetgevingstoets op rechtsstatelijke kwaliteit.',
    col: 1, row: 5, step: 5,
  },
  {
    id: 'signoff',
    branch: 'ministry',
    type: 'review',
    gitLabel: 'approve (internal)',
    lawLabel: 'Akkoord DG/SG',
    subtitle: 'Ambtelijke goedkeuring',
    description:
      'Goedkeuringsketen: wetgevingsjurist → afdelingshoofd → directeur → DG → SG → minister. ' +
      'Vergelijkbaar met required approvals voordat je mag pushen.',
    col: 1, row: 6, step: 6,
  },

  // === Phase B: Interdepartmental ===
  {
    id: 'interdept',
    branch: 'ministry',
    type: 'review',
    gitLabel: 'cross-team review',
    lawLabel: 'Interdepartementaal overleg',
    subtitle: 'IOWJZ / IHW afstemming',
    description:
      'Het concept wordt rondgestuurd naar alle betrokken ministeries. Via IOWJZ en IHW ' +
      'worden commentaren verzameld en geschillen opgelost.',
    col: 1, row: 7, step: 7,
  },
  // Parallel CI checks — all run simultaneously (same row, different columns)
  {
    id: 'uh-toets',
    branch: 'checks',
    type: 'ci-check',
    gitLabel: 'CI: integration test',
    lawLabel: 'Uitvoerbaarheidstoets',
    subtitle: 'UWV / Belastingdienst / SVB',
    description:
      'Uitvoeringsorganisaties toetsen of de wet in de praktijk uitvoerbaar en handhaafbaar is. ' +
      'Loopt parallel met de andere toetsen.',
    col: 3, row: 8, step: 8,
  },
  {
    id: 'regeldruk',
    branch: 'checks',
    type: 'ci-check',
    gitLabel: 'CI: performance test',
    lawLabel: 'Regeldruktoets (ATR)',
    subtitle: 'Administratieve lasten',
    description:
      'Het Adviescollege Toetsing Regeldruk beoordeelt de regeldruk voor burgers en bedrijven. ' +
      'Loopt parallel met de andere toetsen.',
    col: 4, row: 8, step: 8,
  },
  {
    id: 'financieel',
    branch: 'checks',
    type: 'ci-check',
    gitLabel: 'CI: budget check',
    lawLabel: 'Financiële toets',
    subtitle: 'Ministerie van Financiën',
    description:
      'Het ministerie van Financiën beoordeelt de budgettaire gevolgen. ' +
      'Loopt parallel met de andere toetsen.',
    col: 5, row: 8, step: 8,
  },
  {
    id: 'privacy-toets',
    branch: 'checks',
    type: 'ci-check',
    gitLabel: 'CI: security scan',
    lawLabel: 'DPIA / Privacy toets',
    subtitle: 'Autoriteit Persoonsgegevens',
    description:
      'Data Protection Impact Assessment. Loopt parallel met de andere toetsen. ' +
      'Vergelijkbaar met een security scan in CI.',
    col: 6, row: 8, step: 8,
  },
  {
    id: 'voorportaal',
    branch: 'ministry',
    type: 'review',
    gitLabel: 'architecture review',
    lawLabel: 'Ambtelijk voorportaal',
    subtitle: 'Resultaten samenbrengen',
    description:
      'Senior ambtenaren bespreken het voorstel met alle toetsresultaten. ' +
      'Resterende geschillen worden opgelost. Vergelijkbaar met het wachten ' +
      'tot alle parallelle CI checks groen zijn.',
    col: 1, row: 10, step: 10,
  },

  // === Phase C: External Consultation (also parallel) ===
  {
    id: 'internetconsultatie',
    branch: 'checks',
    type: 'review',
    gitLabel: 'public RFC',
    lawLabel: 'Internetconsultatie',
    subtitle: '4+ weken publiek commentaar',
    description:
      'Het concept wordt gepubliceerd op internetconsultatie.nl. Burgers, bedrijven, ' +
      'NGO\'s en experts reageren — minimaal 4 weken.',
    col: 3, row: 11, step: 11,
  },
  {
    id: 'adviesorganen',
    branch: 'checks',
    type: 'ci-check',
    gitLabel: 'CI: domain experts',
    lawLabel: 'Adviesorganen',
    subtitle: 'SER, RvdR, AP, etc.',
    description:
      'Gespecialiseerde organen geven advies: SER, Raad voor de Rechtspraak, ' +
      'Autoriteit Persoonsgegevens. Loopt parallel met internetconsultatie.',
    col: 5, row: 11, step: 11,
  },
  {
    id: 'verwerking-reacties',
    branch: 'ministry',
    type: 'commit',
    gitLabel: 'commit (feedback)',
    lawLabel: 'Verwerking reacties',
    subtitle: 'Aanpassingen n.a.v. consultatie',
    description:
      'Het ministerie verwerkt alle reacties uit consultatie en adviezen. ' +
      'Het voorstel wordt aangepast. Nieuwe commits.',
    col: 1, row: 12, step: 12,
  },

  // === Phase D: Cabinet ===
  {
    id: 'onderraad',
    branch: 'ministry',
    type: 'review',
    gitLabel: 'team lead review',
    lawLabel: 'Onderraad',
    subtitle: 'Relevante ministers',
    description:
      'De relevante ministeriële onderraad bespreekt het voorstel. Politieke afstemming.',
    col: 1, row: 13, step: 13,
  },
  {
    id: 'ministerraad',
    branch: 'ministry',
    type: 'review',
    gitLabel: 'project owner approve',
    lawLabel: 'Ministerraad',
    subtitle: 'Kabinetsbesluit (elke vrijdag)',
    description:
      'De voltallige ministerraad keurt het voorstel goed. Het politieke akkoord.',
    col: 1, row: 14, step: 14,
  },

  // === Phase E: Raad van State ===
  {
    id: 'adviesaanvraag-rvs',
    branch: 'checks',
    type: 'ci-check',
    gitLabel: 'CI: senior review',
    lawLabel: 'Advies Raad van State',
    subtitle: 'Constitutionele toets (art. 73 Gw)',
    description:
      'De Raad van State toetst op grondwettelijkheid, juridische kwaliteit, ' +
      'wetgevingstechniek en beleidseffectiviteit. Dictum: positief/negatief.',
    col: 3, row: 15, step: 15,
  },
  {
    id: 'nader-rapport',
    branch: 'ministry',
    type: 'commit',
    gitLabel: 'commit (address review)',
    lawLabel: 'Nader rapport',
    subtitle: 'Reactie op advies RvS',
    description:
      'De regering reageert op het advies en past het voorstel aan. Het nader rapport ' +
      'beschrijft welke adviezen zijn overgenomen.',
    col: 1, row: 16, step: 16,
  },

  // === Rebase moment ===
  {
    id: 'rebase',
    branch: 'ministry',
    type: 'commit',
    gitLabel: 'git rebase main',
    lawLabel: 'Actualisering wettekst',
    subtitle: 'Verwerking tussentijdse wetswijzigingen',
    description:
      'Tijdens de jaren van voorbereiding is het Corpus Juris veranderd. Het voorstel wordt ' +
      'geactualiseerd: verwijzingen worden bijgewerkt, samenloopbepalingen toegevoegd. ' +
      'Vergelijkbaar met een rebase op main.',
    col: 1, row: 17, step: 17,
  },

  {
    id: 'koninklijke-boodschap',
    branch: 'parliament',
    type: 'branch',
    gitLabel: 'git push + open PR',
    lawLabel: 'Koninklijke Boodschap',
    subtitle: 'Indiening bij Tweede Kamer',
    description:
      'Het wetsvoorstel wordt via een Koninklijke Boodschap ingediend bij de Tweede Kamer. ' +
      'Dit is het moment dat de PR wordt geopend.',
    col: 2, row: 19, step: 18,
  },

  // === Initiatiefwetsvoorstel (external contributor) ===
  {
    id: 'initiatief-voorstel',
    branch: 'initiatief',
    type: 'branch',
    gitLabel: 'external contributor PR',
    lawLabel: 'Initiatiefwetsvoorstel',
    subtitle: 'Voorstel vanuit de Kamer',
    description:
      'Elk Tweede Kamerlid kan zelf een wetsvoorstel indienen — zonder regering. ' +
      'Vergelijkbaar met een external contributor die een PR opent. ' +
      'Het lid verdedigt het voorstel zelf (i.p.v. de minister).',
    col: 5, row: 19, step: 18,
  },
  {
    id: 'initiatief-rvs',
    branch: 'initiatief',
    type: 'ci-check',
    gitLabel: 'CI (via TK)',
    lawLabel: 'RvS advies (via Kamer)',
    subtitle: 'TK vraagt advies',
    description:
      'Bij een initiatiefwetsvoorstel vraagt de Tweede Kamer (niet de regering) advies ' +
      'aan de Raad van State.',
    col: 5, row: 20, step: 19,
  },
  {
    id: 'initiatief-merge',
    branch: 'initiatief',
    type: 'commit',
    gitLabel: 'merge into PR',
    lawLabel: 'Behandeling als regulier voorstel',
    subtitle: 'Zelfde procedure vanaf hier',
    description:
      'Na het RvS advies volgt het initiatiefwetsvoorstel dezelfde parlementaire procedure. ' +
      'Het wordt behandeld als elk ander wetsvoorstel.',
    col: 5, row: 21, step: 20,
  },

  // === Phase F: Tweede Kamer ===
  {
    id: 'tk-verslag',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'PR review comments',
    lawLabel: 'Verslag',
    subtitle: 'Schriftelijke vragen commissie',
    description:
      'De vaste Kamercommissie stelt schriftelijke vragen. Kan meerdere rondes duren.',
    col: 2, row: 21, step: 20,
  },
  {
    id: 'tk-nota-nav',
    branch: 'parliament',
    type: 'commit',
    gitLabel: 'respond to review',
    lawLabel: 'Nota n.a.v. verslag',
    subtitle: 'Beantwoording vragen',
    description:
      'De regering beantwoordt alle vragen uit het verslag.',
    col: 2, row: 22, step: 21,
  },
  {
    id: 'nota-van-wijziging-1',
    branch: 'parliament',
    type: 'commit',
    gitLabel: 'push commits',
    lawLabel: 'Nota van wijziging (1)',
    subtitle: 'Regering wijzigt eigen voorstel',
    description:
      'De regering kan het eigen voorstel wijzigen. Er kunnen meerdere nota\'s van wijziging zijn.',
    col: 2, row: 23, step: 22,
  },
  {
    id: 'nota-van-wijziging-2',
    branch: 'parliament',
    type: 'commit',
    gitLabel: 'push more commits',
    lawLabel: 'Nota van wijziging (2)',
    subtitle: 'Verdere aanpassingen',
    description:
      'Een tweede nota van wijziging — het voorstel evolueert op basis van politieke onderhandeling.',
    col: 2, row: 24, step: 23,
  },
  {
    id: 'wetgevingsoverleg',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'line-by-line review',
    lawLabel: 'Wetgevingsoverleg',
    subtitle: 'Artikelsgewijs in commissie',
    description:
      'Formele commissievergadering: artikelsgewijze bespreking. Alle Kamerleden mogen deelnemen.',
    col: 2, row: 25, step: 24,
  },
  {
    id: 'plenair-debat',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'PR discussion',
    lawLabel: 'Plenair debat',
    subtitle: 'Voltallige Kamer',
    description:
      'Plenair debat: woordvoerders spreken, minister reageert. ' +
      'Amendementen en moties worden ingediend.',
    col: 2, row: 26, step: 25,
  },

  // === Amendments (parallel — filed independently) ===
  {
    id: 'amendement-a',
    branch: 'amendments',
    type: 'commit',
    gitLabel: 'patch/amendement-12',
    lawLabel: 'Amendement Van der Berg',
    subtitle: 'Art. 3 lid 2 wijzigen',
    description:
      'Kamerlid Van der Berg dient een amendement in om artikel 3 lid 2 te wijzigen. ' +
      'Elk Kamerlid kan amendementen indienen — vergelijkbaar met een reviewer-submitted patch.',
    col: 4, row: 28, step: 26,
  },
  {
    id: 'amendement-b',
    branch: 'amendments',
    type: 'commit',
    gitLabel: 'patch/amendement-15',
    lawLabel: 'Amendement Jansen',
    subtitle: 'Nieuw artikel 5a toevoegen',
    description:
      'Kamerlid Jansen dient een amendement in om een nieuw artikel toe te voegen. ' +
      'Dit amendement is onafhankelijk van amendement Van der Berg.',
    col: 5, row: 28, step: 26,
  },
  {
    id: 'amendement-c',
    branch: 'amendments',
    type: 'commit',
    gitLabel: 'patch/amendement-18 (conflicterend)',
    lawLabel: 'Amendement De Vries',
    subtitle: 'Conflicteert met Van der Berg',
    description:
      'Dit amendement wijzigt hetzelfde artikellid als Van der Berg — een merge conflict. ' +
      'Bureau Wetgeving bepaalt de stemvolgorde: verste strekking eerst. ' +
      'Als één wordt aangenomen, vervalt het andere.',
    col: 6, row: 28, step: 26,
  },
  {
    id: 'subamendement-1',
    branch: 'sub-amendments',
    type: 'commit',
    gitLabel: 'patch/sub/amendement-12a',
    lawLabel: 'Subamendement op Van der Berg',
    subtitle: 'Wijziging op het amendement',
    description:
      'Een subamendement: een wijziging op het amendement van Van der Berg. ' +
      'Maximaal één niveau diep — geen sub-subamendementen. ' +
      'Wordt altijd eerst in stemming gebracht.',
    col: 4, row: 30, step: 27,
  },
  {
    id: 'amendement-adopted',
    branch: 'amendments',
    type: 'commit',
    gitLabel: 'cherry-pick (overgenomen)',
    lawLabel: 'Overgenomen amendement',
    subtitle: 'Regering neemt over, geen stemming',
    description:
      'De minister neemt het amendement van Jansen over. Het wordt direct onderdeel ' +
      'van het voorstel — geen stemming nodig. Cherry-pick door de maintainer.',
    col: 5, row: 30, step: 28,
  },
  {
    id: 'stemmingslijst',
    branch: 'parliament',
    type: 'ci-check',
    gitLabel: 'CI: conflict detection',
    lawLabel: 'Stemmingslijst',
    subtitle: 'Bureau Wetgeving ordent',
    description:
      'Bureau Wetgeving stelt de stemmingslijst op: subamendementen eerst, dan amendementen ' +
      '(verste strekking eerst), per artikel. Voorkomt tegenstrijdige tekst. ' +
      'De merge queue met conflict detection.',
    col: 2, row: 33, step: 30,
  },
  {
    id: 'stemmingen',
    branch: 'parliament',
    type: 'merge',
    gitLabel: 'merge queue',
    lawLabel: 'Stemmingen',
    subtitle: 'Per artikel, dan geheel',
    description:
      'Stemming verloopt per artikel (met amendementen), dan over het hele voorstel. ' +
      'Gewone meerderheid, quorum vereist.',
    col: 2, row: 34, step: 31,
  },

  // === Phase G: Eerste Kamer ===
  {
    id: 'ek-behandeling',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'final reviewer',
    lawLabel: 'Eerste Kamer behandeling',
    subtitle: 'Voorlopig verslag + debat',
    description:
      'Schriftelijke voorbereiding gevolgd door plenair debat. ' +
      'De Eerste Kamer heeft GEEN recht van amendement.',
    col: 2, row: 36, step: 32,
  },
  {
    id: 'ek-toezegging',
    branch: 'parliament',
    type: 'commit',
    gitLabel: 'create issue (follow-up)',
    lawLabel: 'Toezegging minister',
    subtitle: 'Belofte voor later',
    description:
      'In plaats van te amenderen, vraagt de Eerste Kamer toezeggingen van de minister: ' +
      'beloftes om bezwaren later op te lossen. Vergelijkbaar met het aanmaken van follow-up issues.',
    col: 2, row: 37, step: 33,
  },
  {
    id: 'ek-stemming',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'approve / reject',
    lawLabel: 'Stemming Eerste Kamer',
    subtitle: 'Aannemen of verwerpen',
    description:
      'Aannemen of verwerpen — zonder wijziging. Protected branch rule: alleen approve/reject.',
    col: 2, row: 38, step: 34,
  },

  // === Novelle (loops back through TK) ===
  {
    id: 'novelle',
    branch: 'delegated',
    type: 'branch',
    gitLabel: 'dependent fix-PR',
    lawLabel: 'Novelle',
    subtitle: 'Reparatietraject terug via TK',
    description:
      'Als de Eerste Kamer bezwaren heeft: een novelle. Een apart wetsvoorstel dat het ' +
      'origineel repareert. Doorloopt het volledige Tweede Kamer-traject opnieuw. ' +
      'Beide worden gelijktijdig aangenomen.',
    col: 6, row: 38, step: 34,
  },
  {
    id: 'novelle-tk',
    branch: 'delegated',
    type: 'review',
    gitLabel: 'PR review (TK, opnieuw)',
    lawLabel: 'Novelle door Tweede Kamer',
    subtitle: 'Volledige behandeling',
    description:
      'De novelle wordt als regulier wetsvoorstel behandeld door de Tweede Kamer. ' +
      'Pas als de TK de novelle aanneemt, kan de EK het origineel behandelen.',
    col: 6, row: 39, step: 35,
  },

  // === Phase H: Publication ===
  {
    id: 'koninklijk-besluit',
    branch: 'main',
    type: 'merge',
    gitLabel: 'merge to main',
    lawLabel: 'Koninklijk Besluit',
    subtitle: 'Bekrachtiging door de Koning',
    description:
      'De Koning bekrachtigt, minister(s) contrasigneren. De feature branch wordt gemerged naar main.',
    col: 0, row: 40, step: 36,
  },
  {
    id: 'staatsblad',
    branch: 'main',
    type: 'deploy',
    gitLabel: 'deploy / publish',
    lawLabel: 'Publicatie Staatsblad',
    subtitle: 'Bekendmaking',
    description:
      'De Minister van JenV plaatst de wet in het Staatsblad. Zonder publicatie geen werking.',
    col: 0, row: 41, step: 37,
  },

  // === Delegated legislation forks off ===
  {
    id: 'amvb-delegation',
    branch: 'delegated',
    type: 'branch',
    gitLabel: 'config/amvb-xyz',
    lawLabel: 'AMvB (gedelegeerd)',
    subtitle: 'Uitwerking bij Koninklijk Besluit',
    description:
      'De wet delegeert regelgeving aan de regering: een AMvB werkt details uit. ' +
      'Gaat NIET door het parlement (tenzij voorhangprocedure). ' +
      'Vergelijkbaar met een config change onder gedelegeerde bevoegdheid.',
    col: 6, row: 42, step: 38,
  },
  {
    id: 'amvb-voorhang',
    branch: 'delegated',
    type: 'review',
    gitLabel: 'voorhang review gate',
    lawLabel: 'Voorhangprocedure',
    subtitle: 'Parlement kijkt mee (4 weken)',
    description:
      'Bij een voorhangprocedure moet de concept-AMvB aan beide Kamers worden voorgelegd ' +
      'voordat deze naar de Raad van State gaat. Het parlement kan debatteren of blokkeren.',
    col: 6, row: 43, step: 39,
  },

  {
    id: 'inwerkingtreding',
    branch: 'main',
    type: 'release',
    gitLabel: 'release / go-live',
    lawLabel: 'Inwerkingtreding',
    subtitle: 'Vaste verandermomenten: 1 jan / 1 jul',
    description:
      'De wet treedt in werking. Verschillende artikelen kunnen op verschillende data ' +
      'in werking treden — gefaseerde rollout. Min. 2 maanden na publicatie.',
    col: 0, row: 44, step: 40,
  },

  // === Concurrent bill (samenloop) ===
  {
    id: 'samenloop',
    branch: 'main',
    type: 'commit',
    gitLabel: 'samenloopbepaling',
    lawLabel: 'Samenloopbepaling',
    subtitle: 'Pre-geprogrammeerde conflictresolutie',
    description:
      'Als twee wetsvoorstellen dezelfde wet wijzigen, bevat elk een samenloopbepaling: ' +
      '"als wet A eerst in werking treedt, pas dan X toe; als wet B eerst, pas dan Y toe." ' +
      'Pre-geprogrammeerde merge conflict resolution — uniek voor wetgeving.',
    col: 0, row: 45, step: 41,
  },

  {
    id: 'corpus-updated',
    branch: 'main',
    type: 'commit',
    gitLabel: 'HEAD',
    lawLabel: 'Corpus Juris',
    subtitle: 'Bijgewerkt',
    description:
      'Het Corpus Juris is bijgewerkt met de nieuwe wet. De main branch gaat door.',
    col: 0, row: 46, step: 42,
  },
];

export const connections = [
  // Branch off from main to ministry
  { from: 'corpus-start', to: 'beleidsidee', type: 'branch-off' },

  // Phase A: Ministry internal (parallel tracks)
  { from: 'beleidsidee', to: 'beleidsnota', type: 'branch-off' },
  { from: 'beleidsidee', to: 'conceptwet', type: 'straight' },
  { from: 'beleidsnota', to: 'mvt', type: 'straight' },
  { from: 'conceptwet', to: 'merge-beleid', type: 'straight' },
  { from: 'mvt', to: 'merge-beleid', type: 'merge-in' },
  { from: 'merge-beleid', to: 'dept-toets', type: 'straight' },
  { from: 'dept-toets', to: 'signoff', type: 'straight' },

  // Phase B: Interdepartmental — parallel CI checks fan out
  { from: 'signoff', to: 'interdept', type: 'straight' },
  { from: 'interdept', to: 'uh-toets', type: 'ci-fork' },
  { from: 'interdept', to: 'regeldruk', type: 'ci-fork' },
  { from: 'interdept', to: 'financieel', type: 'ci-fork' },
  { from: 'interdept', to: 'privacy-toets', type: 'ci-fork' },
  // All return independently
  { from: 'uh-toets', to: 'voorportaal', type: 'ci-return' },
  { from: 'regeldruk', to: 'voorportaal', type: 'ci-return' },
  { from: 'financieel', to: 'voorportaal', type: 'ci-return' },
  { from: 'privacy-toets', to: 'voorportaal', type: 'ci-return' },
  { from: 'interdept', to: 'voorportaal', type: 'straight' },

  // Phase C: External consultation — also parallel
  { from: 'voorportaal', to: 'internetconsultatie', type: 'ci-fork' },
  { from: 'voorportaal', to: 'adviesorganen', type: 'ci-fork' },
  { from: 'internetconsultatie', to: 'verwerking-reacties', type: 'ci-return' },
  { from: 'adviesorganen', to: 'verwerking-reacties', type: 'ci-return' },
  { from: 'voorportaal', to: 'verwerking-reacties', type: 'straight' },

  // Phase D: Cabinet
  { from: 'verwerking-reacties', to: 'onderraad', type: 'straight' },
  { from: 'onderraad', to: 'ministerraad', type: 'straight' },

  // Phase E: Raad van State
  { from: 'ministerraad', to: 'adviesaanvraag-rvs', type: 'ci-fork' },
  { from: 'adviesaanvraag-rvs', to: 'nader-rapport', type: 'ci-return' },
  { from: 'ministerraad', to: 'nader-rapport', type: 'straight' },
  { from: 'nader-rapport', to: 'rebase', type: 'straight' },

  // Transition from ministry to parliament
  { from: 'rebase', to: 'koninklijke-boodschap', type: 'branch-off' },

  // Initiatiefwetsvoorstel (parallel external contributor)
  { from: 'corpus-start', to: 'initiatief-voorstel', type: 'branch-off' },
  { from: 'initiatief-voorstel', to: 'initiatief-rvs', type: 'straight' },
  { from: 'initiatief-rvs', to: 'initiatief-merge', type: 'straight' },
  { from: 'initiatief-merge', to: 'tk-verslag', type: 'merge-in' },

  // Phase F: Tweede Kamer
  { from: 'koninklijke-boodschap', to: 'tk-verslag', type: 'straight' },
  { from: 'tk-verslag', to: 'tk-nota-nav', type: 'straight' },
  { from: 'tk-nota-nav', to: 'nota-van-wijziging-1', type: 'straight' },
  { from: 'nota-van-wijziging-1', to: 'nota-van-wijziging-2', type: 'straight' },
  { from: 'nota-van-wijziging-2', to: 'wetgevingsoverleg', type: 'straight' },
  { from: 'wetgevingsoverleg', to: 'plenair-debat', type: 'straight' },

  // Amendments fan out in parallel from plenair debat
  { from: 'plenair-debat', to: 'amendement-a', type: 'ci-fork' },
  { from: 'plenair-debat', to: 'amendement-b', type: 'ci-fork' },
  { from: 'plenair-debat', to: 'amendement-c', type: 'ci-fork' },
  // Sub-amendment branches further from amendement-a
  { from: 'amendement-a', to: 'subamendement-1', type: 'straight' },
  // Adopted amendment (government cherry-picks)
  { from: 'amendement-b', to: 'amendement-adopted', type: 'straight' },

  // Amendments merge back via stemmingen
  { from: 'subamendement-1', to: 'stemmingslijst', type: 'ci-return' },
  { from: 'amendement-c', to: 'stemmingslijst', type: 'ci-return' },
  { from: 'amendement-adopted', to: 'stemmingslijst', type: 'ci-return' },
  { from: 'plenair-debat', to: 'stemmingslijst', type: 'straight' },
  { from: 'stemmingslijst', to: 'stemmingen', type: 'straight' },

  // Phase G: Eerste Kamer
  { from: 'stemmingen', to: 'ek-behandeling', type: 'straight' },
  { from: 'ek-behandeling', to: 'ek-toezegging', type: 'straight' },
  { from: 'ek-toezegging', to: 'ek-stemming', type: 'straight' },

  // Novelle branches off and loops back
  { from: 'ek-stemming', to: 'novelle', type: 'branch-off' },
  { from: 'novelle', to: 'novelle-tk', type: 'straight' },

  // Phase H: merge to main
  { from: 'ek-stemming', to: 'koninklijk-besluit', type: 'merge-in' },

  // Main continues
  { from: 'corpus-start', to: 'koninklijk-besluit', type: 'main-continues' },
  { from: 'koninklijk-besluit', to: 'staatsblad', type: 'straight' },

  // AMvB branches off after staatsblad
  { from: 'staatsblad', to: 'amvb-delegation', type: 'branch-off' },
  { from: 'amvb-delegation', to: 'amvb-voorhang', type: 'straight' },

  { from: 'staatsblad', to: 'inwerkingtreding', type: 'straight' },
  { from: 'inwerkingtreding', to: 'samenloop', type: 'straight' },
  { from: 'samenloop', to: 'corpus-updated', type: 'straight' },
];
