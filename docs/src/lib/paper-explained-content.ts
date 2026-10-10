/*
 * The text of the plain-language page about the position paper "Rules as
 * Executed": /research/rules-as-executed/uitgelegd and /explained.
 *
 * Written for someone who has never heard of the paper, at language level B1:
 * short sentences, everyday words, one story from start to end. Dutch is the
 * source; the English is its translation. Edit both together (AGENTS.md treats
 * a key in one dataset only as a bug).
 *
 * Every claim here explains the paper as published on 31 August 2026 and has
 * to be traceable to it. The claim-by-claim mapping, with the sentence of the
 * paper that carries each one, is in paper-explained.claims.md next to this
 * file. A simpler sentence may leave detail out; it may not say more than the
 * paper does.
 */

export type Lang = 'nl' | 'en';

export interface Cell {
  /** 'yes' | 'part' | 'no', drawn as a tag. */
  v: 'yes' | 'part' | 'no';
  note?: string;
}

export interface ActorRow {
  actor: string;
  now: [Cell, Cell, Cell];
  proposal: [Cell, Cell, Cell];
}

export interface OverrideVariant {
  id: string;
  label: string;
  short: string;
  rows: [string, string][];
  note: string;
}

export interface Option {
  id: string;
  label: string;
  short: string;
  outcome: string;
  tag: string;
  tone: 'success' | 'critical';
}

export interface FlowStep {
  label: string;
  text: string;
}

export interface Content {
  meta: { title: string; description: string };
  header: {
    title: string;
    subtitle: string;
    lede: string;
    version: (date: string) => string;
    veterans: string;
    tocLabel: string;
    pdf: string;
    paper: string;
    discuss: string;
  };
  readInPaper: string;
  more: string;
  close: string;
  step: string;
  thisPaper: string;
  tableLink: string;
  figureLink: string;
  yes: string;
  part: string;
  no: string;
  now: string;
  proposal: string;
  /** The same, short enough for a two-way switch on a 320 px screen. */
  proposalShort: string;

  /** Step 1: the law is published, what the computer does is not. */
  published: {
    title: string;
    p1: string;
    caseLabel: string;
    caseIntro: string;
    options: Option[];
    caseNote: string;
    caseLink: { href: string; text: string };
    demandLabel: string;
    demand: string;
    more: string[];
  };
  /** Step 2: who can look along. */
  power: {
    title: string;
    p1: string;
    matrixLabel: string;
    rowHeader: string;
    columns: [string, string, string];
    more: string[];
  };
  /** Step 3: the proposal. */
  binding: {
    title: string;
    p1: string;
    flowLabel: string;
    flowNow: FlowStep[];
    flowNowNote: string;
    flowProposal: FlowStep[];
    flowProposalNote: string;
    more: string[];
    exampleLabel: string;
    exampleStatute: string;
    mapRows: [string, string, string][];
    exampleNote: string;
  };
  /** Step 4: the receipt, computed in the browser. */
  receipt: {
    title: string;
    p1: string;
    p2: string;
    demoIntro: string;
    decision: string;
    fields: { law: string; date: string; inputs: string; amount: string; digest: string };
    inputsSummary: string;
    inForce: string;
    check: string;
    checking: string;
    checkDigest: string;
    checkAmount: string;
    ok: string;
    mismatch: string;
    loading: string;
    failed: string;
    noScript: string;
    revealA: string;
    revealB: (published: string, local: string, diff: string) => string;
    limitsLabel: string;
    limits: string[];
    more: string[];
  };
  /** Step 5: a person may depart from the outcome. */
  default: {
    title: string;
    p1: string;
    variantsLabel: string;
    variants: OverrideVariant[];
    more: string[];
  };
  /** Step 6: what this does not fix. */
  limits: {
    title: string;
    intro: string[];
    /** What the proposal writes out: where there is room, where not, and that it was used. */
    explicit: [string, string][];
    explicitNote: string;
    p2: string;
    openLabel: string;
    open: string[];
    more: string[];
  };
  /** Step 7: for people who have worked on rules as code for years. */
  veterans: {
    title: string;
    p1: string;
    p2: string;
    tableLabel: string;
    rowHeader: string;
    columns: [string, string, string];
    /** What each column of the comparison means, with the paper's own term. */
    columnHelp: [string, string, string];
    /** Under the table: why only this paper has three yeses. */
    tableNote: string;
    p3: string;
    more: string[];
    reasons: string[];
    moreAfter: string[];
  };
}

const nl: Content = {
  meta: {
    title: 'Rules as Executed, eenvoudig uitgelegd · RegelRecht',
    description:
      'Het paper Rules as Executed in gewone taal: waarom de regels die een computer gebruikt openbaar moeten zijn, en hoe je een besluit dan zelf narekent.',
  },
  header: {
    title: 'Rules as Executed, eenvoudig uitgelegd',
    subtitle: 'Het paper in gewone taal',
    lede:
      'Steeds vaker rekent een computer uit wat je krijgt of moet betalen, bijvoorbeeld bij de zorgtoeslag. De wet waar dat op rust, kan iedereen lezen. Wat de computer precies met die wet doet, kan bijna niemand zien. Het paper Rules as Executed stelt voor om dat te veranderen. Op deze pagina lees je hoe, in zeven stappen.',
    version: (date) =>
      `Dit is een uitleg bij het paper zoals het op ${date} verscheen. Wil je het precies weten, lees dan het paper zelf.`,
    veterans: 'Werk je al jaren met regels als code? Ga dan naar stap 7.',
    tocLabel: 'De zeven stappen',
    pdf: 'Pdf op Zenodo',
    paper: 'Lees het paper',
    discuss: 'Praat mee over het paper',
  },
  readInPaper: 'In het paper',
  more: 'Meer uit het paper',
  close: 'Sluiten',
  step: 'Stap',
  thisPaper: 'Dit paper',
  tableLink: 'Dit overzicht in het paper',
  figureLink: 'Dit voorbeeld in het paper',
  yes: 'ja',
  part: 'deels',
  no: 'nee',
  now: 'Nu',
  proposal: 'Met het voorstel',
  proposalShort: 'Met voorstel',

  published: {
    title: 'De wet staat online, wat de computer doet niet',
    p1:
      'Een wet zegt wie ergens recht op heeft. Een computer heeft meer nodig. Hij moet weten welke gegevens hij gebruikt, hoe hij een begrip uit de wet invult en wat hij doet als een geval net anders ligt. Die keuzes maakt de organisatie die de wet uitvoert. Een deel schrijft ze op in regels die je kunt opzoeken. Een groot deel zit alleen in werkinstructies en in de software zelf.',
    caseLabel: 'Een voorbeeld: wie is je toeslagpartner?',
    caseIntro:
      'Je huurt samen met een vriend een huis. Jullie staan al twee jaar op hetzelfde adres ingeschreven, maar jullie zijn geen stel. Je vraagt zorgtoeslag aan. Telt je huisgenoot als je toeslagpartner?',
    options: [
      {
        id: 'adres',
        label: 'Het systeem kijkt alleen naar het adres',
        short: 'Adres',
        outcome:
          'Jullie staan langer dan zes maanden op hetzelfde adres. Het systeem ziet je huisgenoot als toeslagpartner en telt het inkomen van je huisgenoot mee. Je krijgt minder of geen zorgtoeslag.',
        tag: 'minder of geen toeslag',
        tone: 'critical',
      },
      {
        id: 'huishouden',
        label: 'Het systeem kijkt ook naar het huishouden',
        short: 'Huishouden',
        outcome:
          'Jullie wonen op hetzelfde adres, maar voeren geen gezamenlijke huishouding. Je huisgenoot is dan geen toeslagpartner. Je krijgt zorgtoeslag op je eigen inkomen.',
        tag: 'toeslag op eigen inkomen',
        tone: 'success',
      },
    ],
    caseNote:
      'De wet zegt onder meer dat een huisgenoot je partner is als jullie langer dan zes maanden op hetzelfde adres staan en een gezamenlijke huishouding voeren. Het adres staat in een registratie, het huishouden niet. Hoe een systeem dat tweede deel invult, bepaalt of je toeslag krijgt, en dat lees je niet in de wet. Het paper noemt juist deze afleiding van een partner uit registraties als voorbeeld. De twee systemen hier zijn bedacht, de wettekst niet.',
    caseLink: { href: 'https://wetten.overheid.nl/BWBR0018472/2025-01-01#Artikel3', text: 'Artikel 3 van de Algemene wet inkomensafhankelijke regelingen' },
    demandLabel: 'Wat het paper vraagt',
    demand: 'Het moet onafhankelijk te controleren zijn welke regels de computer echt gebruikt.',
    more: [
      'Het paper noemt dit een vierde eis van de rechtsstaat. De eerste drie bestaan al lang: regels moeten te vinden zijn, te begrijpen en te voorspellen. Die eisen gingen ervan uit dat je kon nagaan hoe een ambtenaar redeneerde. Nu computers de wet uitvoeren, moet je ook kunnen controleren wat die computers doen.',
      'Een besluit hangt vaak niet alleen af van de wet zelf. Het hangt ook af van hoe de organisatie die wet uitlegt. Een deel van die uitleg staat in beleidsregels, en die moeten openbaar zijn. Een groot deel staat nergens dan in de organisatie zelf.',
    ],
  },

  power: {
    title: 'Wie kan meekijken?',
    p1:
      'Op dit moment kan alleen de organisatie die de computer gebruikt zien wat hij doet. Ook die organisatie overziet het niet altijd: software groeit jaren door, en soms is ze van een leverancier, en mag de overheid niet in de code kijken. Het parlement stemt over de wet, maar ziet niet hoe die in de praktijk uitpakt. Een rechter krijgt de uitkomst te zien. Wie het besluit krijgt, kan niet nagaan of het klopt.',
    matrixLabel: 'Wie kan wat?',
    rowHeader: 'Wie',
    columns: ['Leest de regels', 'Rekent één besluit na', 'Rekent veel gevallen door'],
    more: [
      'Met het voorstel kun je je eigen besluit narekenen, maar niet dat van je buren. Dat is met opzet. Als iedereen elk besluit kon narekenen, zou je openheid kopen door privacy op te geven, schrijft het paper.',
      'De Tweede Kamer heeft geen gegevens van mensen nodig. Ze krijgt de regels zelf, en kan zien wat een wetswijziging zou doen voordat ze erover stemt. Kamerleden hoeven die regels niet zelf te lezen. Het paper denkt aan de ondersteuning van de Kamer, die dat voor alle fracties kan doen. In Frankrijk rekent het team LexImpact van de Assemblée nationale op die manier voorstellen voor Kamerleden door.',
      'Ook wie een wet voorbereidt, kan eerder zien wat die doet. Een beleidsmaker kan een nieuwe regel doorrekenen voordat die wordt ingediend, en zien of dezelfde uitkomst ook met minder gegevens over mensen te bereiken is. Een wetgevingsjurist kan de rekenvoorbeelden uit de memorie van toelichting als toets gebruiken: het regelwerk geeft dezelfde uitkomst, of niet. Dat werkt alleen als die voorbeelden worden bijgehouden, want na een wetswijziging of indexering kloppen ze vaak niet meer.',
      'De overheid heeft nu andere controles. Het Algoritmeregister noemt systemen, maar laat de regels zelf zelden zien. Commissies en toetsen zoals een DPIA beoordelen vooral beschrijvingen, gemaakt door wie de systemen bouwde. Met gepubliceerde regels kunnen ze de regels zelf beoordelen.',
    ],
  },

  binding: {
    title: 'Het voorstel: publiceer wat de computer doet',
    p1:
      'Het paper stelt voor om de regels die de computer gebruikt te publiceren, net als de wet zelf. Ze staan dan in een eenvoudige vorm die bedoeld is om door juristen gelezen te worden en die een computer kan uitvoeren. Zo’n vastgelegde regeling heet een regelwerk (het paper zelf spreekt nog van een executable specification). De organisatie moet daarna beslissen met precies die gepubliceerde versie, en met geen andere.',
    flowLabel: 'Van wet naar besluit',
    flowNow: [
      { label: 'De wet', text: 'Iedereen kan hem lezen' },
      { label: 'Beleidsregels', text: 'Deels te vinden' },
      { label: 'Werkinstructies', text: 'Niet te zien' },
      { label: 'De software', text: 'Niet te zien' },
      { label: 'Het besluit', text: 'Je ziet niet welke regels beslisten' },
    ],
    flowNowNote:
      'Elke stap is een vertaling van de vorige, en mensen houden ze met de hand gelijk. Niets garandeert dat de laatste nog klopt met de eerste.',
    flowProposal: [
      { label: 'De wet', text: 'Iedereen kan hem lezen' },
      { label: 'Het regelwerk', text: 'Gepubliceerd, en de enige versie die de computer gebruikt' },
      { label: 'Het besluit', text: 'Met een bonnetje erbij (stap 4)' },
    ],
    flowProposalNote: 'De keuzes die het besluit bepalen, staan nu op één plek, en iedereen kan ze lezen.',
    more: [
      'De vorm is met opzet eenvoudig. Er kan in gerekend en vergeleken worden, en datums kunnen worden getoetst, maar veel meer niet. Daardoor stopt elke berekening, en blijft het te lezen voor juristen. De regels volgen de artikelen van de wet, zodat je per artikel kunt nagaan of ze kloppen. Of juristen dat echt zonder hulp kunnen, moet volgens het paper nog blijken.',
      'Een regelwerk is een uitleg van de wet, niet de wet zelf. Als het botst met de wet, gaat de wet voor en moet het regelwerk worden aangepast.',
      'De uitleg blijft het werk van de organisatie die de wet uitvoert. Die keuzes werden al gemaakt, schrijft het paper, alleen zag niemand buiten de organisatie ze. Na publicatie zijn ze zichtbaar, en kun je iemand erop aanspreken.',
    ],
    exampleLabel: 'Hoe een regelwerk eruitziet (naar een verzonnen voorbeeld uit het paper)',
    exampleStatute:
      'Een bewoner komt in aanmerking voor een parkeervergunning als zijn voertuig weinig uitstoot. Voor een emissievrij voertuig geldt een lager tarief.',
    mapRows: [
      ['Een bewoner', 'is_resident', 'uit de basisregistratie'],
      ['als zijn voertuig weinig uitstoot', 'is_low_emission', 'uit het kentekenregister'],
      ['komt in aanmerking voor een parkeervergunning', 'eligible = AND(is_resident, is_low_emission)', 'dit artikel rekent het uit'],
      [
        'Voor een emissievrij voertuig geldt een lager tarief',
        'permit_fee = IF is_zero_emission THEN reduced_fee ELSE standard_fee',
        'uit de tarieventabel',
      ],
    ],
    exampleNote: 'Elk stukje van het artikel krijgt zijn eigen regel, en bij elke waarde staat waar die vandaan komt.',
  },

  receipt: {
    title: 'Het bonnetje',
    p1:
      'Met het voorstel hoort bij elk besluit een bonnetje. Daarop staat welke versie van de regels de organisatie zegt te hebben gebruikt, met welke gegevens hij rekende en wat eruit kwam. De versie staat er als een soort vingerafdruk op: een code die verandert zodra er ook maar iets in de regels anders is.',
    p2:
      'Met dat bonnetje kun je het besluit zelf narekenen. Je neemt de gepubliceerde regels, stopt dezelfde gegevens erin en kijkt of hetzelfde bedrag eruit komt. Dezelfde regels met dezelfde gegevens geven altijd dezelfde uitkomst.',
    demoIntro:
      'Probeer het maar. Hieronder staan twee besluiten over de zorgtoeslag van dezelfde persoon, met testgegevens. Je browser heeft ze net uitgerekend. Klik bij allebei op Reken na.',
    decision: 'Besluit',
    fields: {
      law: 'Regels',
      date: 'Rekendatum',
      inputs: 'Gegevens',
      amount: 'Zorgtoeslag volgens het besluit',
      digest: 'Vingerafdruk van de regels',
    },
    inputsSummary: '{n} gegevens uit de registraties, dezelfde voor beide besluiten',
    inForce: 'geldig vanaf {date}',
    check: 'Reken na',
    checking: 'Bezig met narekenen…',
    checkDigest: 'Dezelfde regels als gepubliceerd',
    checkAmount: 'Hetzelfde bedrag als bij narekenen',
    ok: 'klopt',
    mismatch: 'klopt niet',
    loading: 'De regels worden opgehaald…',
    failed: 'Narekenen lukte niet. Laad de pagina opnieuw.',
    noScript: 'Hiervoor is JavaScript nodig: het narekenen gebeurt in je eigen browser.',
    revealA:
      'Beide controles kloppen. De regels op het bonnetje zijn de gepubliceerde, en narekenen geeft hetzelfde bedrag.',
    revealB: (published, local, diff) =>
      `Dit klopt niet. Het systeem achter dit besluit gebruikte een eigen kopie van de regels. Daarin stond één percentage nog op de waarde van vorig jaar: ${local} in plaats van ${published}. Die kopie is nooit gepubliceerd. Het scheelt ${diff}, en je ziet het op twee manieren: de vingerafdruk is anders, en narekenen geeft een ander bedrag.`,
    limitsLabel: 'Wat het bonnetje niet laat zien',
    limits: [
      'Het bonnetje laat zien met welke gegevens is gerekend. Of die gegevens kloppen, zie je er niet aan.',
      'Of je nu al recht hebt op zo’n bonnetje, is nog onduidelijk. Het paper stelt voor dat je het altijd krijgt, samen met het besluit.',
    ],
    more: [
      'Beide besluiten zijn echt uitgerekend, in je browser, met de regels die deze site gebruikt. Voor besluit B past deze pagina zelf een kopie van de regels aan. Een echt bonnetje is ook ondertekend door de organisatie. Deze pagina ondertekent niets.',
      'Op een echt bonnetje staat ook welk programma de berekening deed. Rekent het programma waarmee jij narekent anders, dan kan er ook een verschil uitkomen. Hoe je zeker weet dat twee programma’s hetzelfde rekenen, is volgens het paper nog een open vraag.',
      'Publiceren en het bonnetje hebben pas gevolgen als ze in de wet staan. Het paper vindt dat een besluit zonder bonnetje dan zelf een gebrek heeft. Welk gebrek precies, laat het aan juristen.',
      'De voorbeelden op deze pagina rekenen met de huidige versie van de software. Die is na het paper verder ontwikkeld, dus een bedrag kan afwijken van wat dezelfde software in augustus 2026 gaf.',
    ],
  },

  default: {
    title: 'Afwijken mag, waar de wet het toestaat',
    p1:
      'De computer geeft een standaarduitkomst. Geeft de wet ruimte om daarvan af te wijken, dan mag dat. Bijvoorbeeld als de uitkomst in een bepaald geval te hard uitvalt. Op het bonnetje staat dan wat er is veranderd, waarom en door wie.',
    variantsLabel: 'Drie manieren van afwijken',
    variants: [
      {
        id: 'lex',
        label: 'De wet zelf wijkt af',
        short: 'De wet',
        rows: [
          ['Wat', 'de termijn voor bezwaar'],
          ['Volgens de algemene regel', '6 weken (Algemene wet bestuursrecht)'],
          ['Wordt', '4 weken'],
          ['Waarom', 'de Vreemdelingenwet 2000 zegt dat, in artikel 69'],
        ],
        note: 'Een speciale wet wijkt af van de algemene. Dat staat al in de gepubliceerde regels.',
      },
      {
        id: 'orgaan',
        label: 'De burgemeester wijkt af in één geval',
        short: 'Eén geval',
        rows: [
          ['Wat', 'hoe lang een woning dicht moet'],
          ['Volgens de regel', '6 maanden'],
          ['Wordt', '3 maanden'],
          ['Waarom', 'zes maanden zou te zwaar zijn voor de kinderen in het gezin'],
          ['Wie', 'de burgemeester'],
        ],
        note: 'Iemand bekijkt het geval en wijkt af. Dat mag alleen waar de wet die ruimte geeft.',
      },
      {
        id: 'batch',
        label: 'Een correctie voor een hele groep',
        short: 'Een groep',
        rows: [
          ['Wat', 'de uitkomst die de regels gaven'],
          ['Wordt', 'wat de rechter heeft bepaald'],
          ['Waarom', 'een uitspraak over een hele groep besluiten'],
          ['Wie', 'de organisatie, voor alle besluiten tegelijk'],
        ],
        note: 'Het bonnetje zegt dan ook dat een programma dit deed, en geen mens die het dossier las.',
      },
    ],
    more: [
      'Gebeurt zo’n afwijking steeds weer automatisch, bij iedereen met een bepaalde eigenschap, dan is de gepubliceerde regel eigenlijk vervangen. Het verschil met nu is dat een controleur het kan vinden, want het staat op elk bonnetje.',
      'De voorbeelden komen uit het paper. Het voorbeeld van de woning is verzonnen. De bezwaartermijn uit de Vreemdelingenwet is echt.',
    ],
  },

  limits: {
    title: 'Wat dit niet oplost',
    intro: [
      'Zolang mensen de wet uitvoerden, was er speelruimte die nergens op papier stond. Een ambtenaar hielp iemand verder terwijl geen regel dat toestond, en niemand keek ernaar. Dat mocht eigenlijk niet, hoe goed het ook was. Maar het geval liep er beter door af dan volgens de regel.',
      'Software heeft die speelruimte niet. Ze doet wat is vastgelegd en verder niets. Dat geldt nu al voor de systemen die de overheid gebruikt, en het heeft niets met publiceren te maken: een regelwerk dat niemand kan inzien, is even streng.',
      'Het voorstel brengt die ongeschreven ruimte niet terug, en dat kan ook niet, want een uitzondering die je opschrijft is gewoon weer een regel. Wel geeft het een deel van de ruimte terug, want de uitkomst van de computer is alleen een standaard. En het maakt zichtbaar wat nu nergens staat:',
    ],
    explicit: [
      [
        'Waar ruimte is',
        'Geeft de wet de overheid ruimte om af te wijken, dan staat dat in het gepubliceerde regelwerk. Ook de gronden die de Algemene wet bestuursrecht daarvoor geeft, staan erin.',
      ],
      ['Waar geen ruimte is', 'Geeft de wet die ruimte niet, dan biedt het regelwerk ook niets om mee af te wijken.'],
      [
        'Dat er is afgeweken',
        'Elke afwijking staat op het bonnetje: wat de regels uitrekenden, wat het werd, op welke grond en wie het deed. Ook als een programma het deed en geen mens (stap 5).',
      ],
    ],
    explicitNote: 'Zo zie je hoe streng de regels zijn, en wie welke keuze maakte.',
    p2: 'Het paper noemt zijn aanpak onvolmaakt, en een aantal vragen laat het open.',
    openLabel: 'Open vragen',
    open: [
      'Wat voor juridisch document een regelwerk eigenlijk is.',
      'Of je recht hebt op het bonnetje, en wat je dan mag zien over anderen, zoals het inkomen van je partner.',
      'Of juristen de regels zonder hulp goed kunnen controleren.',
      'Hoe je zeker weet dat twee programma’s hetzelfde uitrekenen.',
      'Wie wat moet bewijzen als een besluit niet klopt met de gepubliceerde regels.',
    ],
    more: [
      'Een bekende zorg is dat mensen regels gaan ontwijken als ze die kennen. Het paper maakt daarom onderscheid. Alleen de regels die een besluit uitrekenen worden openbaar. Hoe de overheid kiest wie ze controleert, hoort daar niet bij. Wie een adviseur kan betalen, kent de drempels nu ook al.',
      'Er was ook ruimte bij de controle. Niet elke regel werd bij iedereen nagelopen, omdat daar de tijd niet voor was. Een systeem dat iedereen steeds automatisch controleert, haalt die ruimte weg. Het paper zegt erbij dat het zelf hoort bij de beweging die die ruimte kleiner maakt.',
      'Een systeem dat je kunt controleren, krijgt misschien juist meer taken. Ook dat schrijft het paper.',
    ],
  },

  veterans: {
    title: 'Voor wie al jaren met regels als code werkt',
    p1:
      'Wetten als regels voor de computer opschrijven is niet nieuw. Denk aan RegelSpraak bij de Belastingdienst, FLINT bij de IND, OpenFisca en Catala in Frankrijk en Better Rules in Nieuw-Zeeland. Ze helpen specialisten om regels te schrijven en te laten rekenen.',
    p2:
      'Het paper vraagt iets anders. De gepubliceerde regels moeten gegarandeerd de regels zijn die het besluit nemen, en wie het besluit krijgt moet dat kunnen nagaan. Dat was voor geen van deze systemen het doel. Het paper zegt er daarom bij dat het overzicht hieronder geen ranglijst is.',
    tableLabel: 'Eerdere aanpakken vergeleken',
    rowHeader: 'Aanpak',
    columns: ['Rekent', 'Openbaar', 'Gebonden'],
    columnHelp: [
      'Een computer kan met de regels rekenen (in het paper: executable).',
      'Iedereen kan de regels lezen (published).',
      'Het is gegarandeerd dat de gepubliceerde regels ook de regels zijn die het besluit nemen (bound to execution). Dit is waar het paper om vraagt.',
    ],
    tableNote:
      'Dat alleen dit paper drie keer ja heeft, is geen toeval. De laatste kolom is precies waar het paper over gaat, en de reden dat het geschreven is. De andere aanpakken zijn gebouwd om regels te schrijven en uit te voeren, en zijn daar goed in. Dit voorstel gaat over wie de regels achteraf wil lezen en controleren.',
    p3:
      'Een voorbeeld dichter bij huis is de Omgevingswet. Daar staan regels machineleesbaar online, en ze sturen de vergunningcheck in het Omgevingsloket. Toch zijn de wet, de regels in het loket en de software van de gemeente drie losse dingen, die mensen met de hand gelijk houden. Het voorstel maakt er één ding van.',
    more: [
      'Het paper kiest niet voor een gecontroleerde natuurlijke taal zoals RegelSpraak, en geeft daar drie redenen voor:',
    ],
    reasons: [
      'Meerdere programma’s moeten hetzelfde uitrekenen. Een tekst die op gewone taal lijkt, kunnen twee programma’s verschillend lezen zonder dat een van beide aantoonbaar fout zit.',
      'Bij een wetswijziging moet precies te zien zijn wat er veranderde. In gewone taal kan een andere formulering hetzelfde lijken en toch iets anders doen.',
      'Het probleem waar het paper over gaat, zit bij het lezen, niet bij het schrijven. RegelSpraak is gemaakt voor wie de regel opstelt, dit voorstel voor wie hem achteraf wil controleren.',
    ],
    moreAfter: [
      'Een laag in gewone taal die naar dit formaat vertaalt, sluit het paper niet uit. En het zegt erbij dat het deze keuze met argumenten verdedigt, niet met metingen.',
      'In de huidige software vertalen taalmodellen wetten naar dit formaat, artikel voor artikel. Ze rekenen nooit zelf. Een vast programma rekent het besluit uit, met regels die mensen hebben nagekeken en die de organisatie heeft overgenomen.',
    ],
  },
};

const en: Content = {
  meta: {
    title: 'Rules as Executed, explained simply · RegelRecht',
    description:
      'The paper Rules as Executed in plain language: why the rules a computer uses should be public, and how you could then check a decision yourself.',
  },
  header: {
    title: 'Rules as Executed, explained simply',
    subtitle: 'The paper in plain language',
    lede:
      'More and more often a computer works out what you get or have to pay, for instance with the Dutch healthcare allowance. Anyone can read the law behind it. Almost nobody can see what the computer actually does with that law. The paper Rules as Executed proposes to change that. This page explains how, in seven steps.',
    version: (date) =>
      `This explains the paper as published on ${date}. For the exact argument, read the paper itself.`,
    veterans: 'Been working on rules as code for years? Go to step 7.',
    tocLabel: 'The seven steps',
    pdf: 'PDF on Zenodo',
    paper: 'Read the paper',
    discuss: 'Discuss the paper',
  },
  readInPaper: 'In the paper',
  more: 'More from the paper',
  close: 'Close',
  step: 'Step',
  thisPaper: 'This paper',
  tableLink: 'This comparison in the paper',
  figureLink: 'This example in the paper',
  yes: 'yes',
  part: 'partly',
  no: 'no',
  now: 'Now',
  proposal: 'With the proposal',
  proposalShort: 'With proposal',

  published: {
    title: 'The law is online, what the computer does is not',
    p1:
      'A law says who is entitled to something. A computer needs more than that. It has to know which data to use, how to fill in a term from the law and what to do when a case is slightly different. The organization that carries out the law makes those choices. Some of them it writes down in rules you can look up. Many live only in work instructions and in the software itself.',
    caseLabel: 'An example: who is your allowance partner?',
    caseIntro:
      'You rent a house together with a friend. You have both been registered at the same address for two years, but you are not a couple. You apply for healthcare allowance. Does your housemate count as your allowance partner?',
    options: [
      {
        id: 'adres',
        label: 'The system only looks at the address',
        short: 'Address',
        outcome:
          'You have been at the same address for more than six months. The system treats your housemate as your allowance partner and counts your housemate’s income too. You get less allowance, or none.',
        tag: 'less or no allowance',
        tone: 'critical',
      },
      {
        id: 'huishouden',
        label: 'The system also looks at the household',
        short: 'Household',
        outcome:
          'You live at the same address, but you do not run a joint household. Your housemate is then not your allowance partner. You get allowance on your own income.',
        tag: 'allowance on own income',
        tone: 'success',
      },
    ],
    caseNote:
      'Among other things, the law says a housemate is your partner if you have been at the same address for more than six months and run a joint household. The address is in a register, the household is not. How a system fills in that second part decides whether you get the allowance, and you will not read it in the law. The paper names exactly this derivation of a partner from registers as an example. The two systems here are made up, the text of the law is not.',
    caseLink: { href: 'https://wetten.overheid.nl/BWBR0018472/2025-01-01#Artikel3', text: 'Article 3 of the Dutch General Act on Income-Related Schemes (in Dutch)' },
    demandLabel: 'What the paper asks',
    demand: 'It must be possible to check independently which rules the computer really uses.',
    more: [
      'The paper calls this a fourth demand of the rule of law. The first three have been around for a long time: rules must be findable, understandable and predictable. Those demands assumed you could examine how an official reasoned. Now that computers carry out the law, you also need to be able to check what those computers do.',
      'A decision often depends on more than the law itself. It also depends on how the organization reads that law. Part of that reading is in policy rules, and those have to be public. A large part exists nowhere but inside the organization.',
    ],
  },

  power: {
    title: 'Who can look along?',
    p1:
      'Right now only the organization that runs the computer can see what it does. Even that organization does not always have the full picture: software grows for years, and sometimes it belongs to a supplier and the government may not look at the code. Parliament votes on the law but does not see how it works out in practice. A court gets to see the outcome. Whoever receives the decision cannot check whether it is right.',
    matrixLabel: 'Who can do what?',
    rowHeader: 'Who',
    columns: ['Reads the rules', 'Checks one decision', 'Runs many cases'],
    more: [
      'With the proposal you can re-run your own decision, but not your neighbor’s. That is on purpose. If everyone could re-run every decision, you would buy openness by giving up privacy, the paper says.',
      'Parliament does not need anyone’s personal data. It gets the rules themselves, and can see what a change to the law would do before voting on it. Members of parliament do not have to read those rules themselves. The paper has in mind the support staff of the House, who could do it for every parliamentary group. In France, the LexImpact team at the Assemblée nationale already works out proposals for deputies this way.',
      'Whoever prepares a law can also see earlier what it does. A policymaker can run a new rule before it is submitted, and see whether the same outcome can be reached with less data about people. A legislative drafter can use the worked examples in the explanatory memorandum as a test: the rulework gives the same outcome, or it does not. That only works if those examples are kept up to date, because after an amendment or indexation they often no longer match.',
      'Government now has other checks. The Dutch Algorithm Register lists systems but rarely shows the rules themselves. Committees and assessments such as a DPIA mostly review descriptions, written by the people who built the systems. With published rules they can assess the rules themselves.',
    ],
  },

  binding: {
    title: 'The proposal: publish what the computer does',
    p1:
      'The paper proposes publishing the rules the computer uses, just like the law itself. They would be written in a simple form meant to be read by lawyers and run by a computer. A regulation recorded this way is called a rulework (the paper itself still says executable specification). The organization then has to decide with exactly that published version, and with no other.',
    flowLabel: 'From law to decision',
    flowNow: [
      { label: 'The law', text: 'Anyone can read it' },
      { label: 'Policy rules', text: 'Partly findable' },
      { label: 'Work instructions', text: 'Cannot be seen' },
      { label: 'The software', text: 'Cannot be seen' },
      { label: 'The decision', text: 'You cannot see which rules decided' },
    ],
    flowNowNote:
      'Each step translates the one before, and people keep them in line by hand. Nothing guarantees that the last one still matches the first.',
    flowProposal: [
      { label: 'The law', text: 'Anyone can read it' },
      { label: 'The rulework', text: 'Published, and the only version the computer uses' },
      { label: 'The decision', text: 'With a receipt attached (step 4)' },
    ],
    flowProposalNote: 'The choices that decide the outcome are now in one place, and anyone can read them.',
    more: [
      'The form is simple on purpose. It can calculate, compare and check dates, and not much more. That way every calculation ends, and lawyers can still read it. The rules follow the articles of the law, so you can check article by article whether they are right. Whether lawyers can really do that without help still has to be shown, according to the paper.',
      'A rulework is a reading of the law, not the law itself. If it clashes with the law, the law wins and the rulework has to be fixed.',
      'The reading remains the work of the organization that carries out the law. Those choices were already being made, the paper says, only nobody outside the organization could see them. Once published, they are visible, and someone can be held to them.',
    ],
    exampleLabel: 'What a rulework looks like (after a made-up example in the paper)',
    exampleStatute:
      'A resident is eligible for a parking permit if their vehicle has low emissions. The fee is reduced for zero-emission vehicles.',
    mapRows: [
      ['A resident', 'is_resident', 'from the population register'],
      ['if their vehicle has low emissions', 'is_low_emission', 'from the vehicle register'],
      ['is eligible for a parking permit', 'eligible = AND(is_resident, is_low_emission)', 'worked out by this article'],
      [
        'The fee is reduced for zero-emission vehicles',
        'permit_fee = IF is_zero_emission THEN reduced_fee ELSE standard_fee',
        'from the fee schedule',
      ],
    ],
    exampleNote: 'Each part of the article gets its own rule, and every value says where it comes from.',
  },

  receipt: {
    title: 'The receipt',
    p1:
      'With the proposal, every decision comes with a receipt. It says which version of the rules the organization says it used, which data it worked with and what came out. The version is on it as a kind of fingerprint: a code that changes as soon as anything in the rules is different.',
    p2:
      'With that receipt you can re-run the decision yourself. You take the published rules, put in the same data and see whether the same amount comes out. The same rules with the same data always give the same result.',
    demoIntro:
      'Try it. Below are two decisions on the healthcare allowance of the same person, with test data. Your browser has just worked them out. Click Check it on both.',
    decision: 'Decision',
    fields: {
      law: 'Rules',
      date: 'Calculation date',
      inputs: 'Data',
      amount: 'Allowance according to the decision',
      digest: 'Fingerprint of the rules',
    },
    inputsSummary: '{n} values from the registers, the same for both decisions',
    inForce: 'in force from {date}',
    check: 'Check it',
    checking: 'Checking…',
    checkDigest: 'The same rules as published',
    checkAmount: 'The same amount when re-run',
    ok: 'matches',
    mismatch: 'does not match',
    loading: 'Fetching the rules…',
    failed: 'The check failed. Reload the page.',
    noScript: 'This needs JavaScript: the check runs in your own browser.',
    revealA: 'Both checks pass. The rules on the receipt are the published ones, and re-running gives the same amount.',
    revealB: (published, local, diff) =>
      `This does not match. The system behind this decision used its own copy of the rules. In it, one percentage still had last year’s value: ${local} instead of ${published}. That copy was never published. The difference is ${diff}, and it shows in two ways: the fingerprint is different, and re-running gives a different amount.`,
    limitsLabel: 'What the receipt does not show',
    limits: [
      'The receipt shows which data were used. It does not show whether those data are correct.',
      'Whether you already have a right to such a receipt is unclear. The paper proposes that you always get it, together with the decision.',
    ],
    more: [
      'Both decisions were really worked out, in your browser, with the rules this site uses. For decision B this page alters a copy of the rules itself. A real receipt is also signed by the organization. This page signs nothing.',
      'A real receipt also says which program did the calculation. If the program you re-run with calculates differently, a difference can come out as well. How to be sure that two programs calculate the same thing is still an open question, according to the paper.',
      'Publishing and the receipt only have consequences once they are in the law. The paper holds that a decision without a receipt then has a defect itself. Which defect exactly, it leaves to lawyers.',
      'The examples on this page use the current version of the software. It has moved on since the paper, so an amount can differ from what the same software gave in August 2026.',
    ],
  },

  default: {
    title: 'Departing is allowed where the law allows it',
    p1:
      'The computer gives a default outcome. If the law leaves room to depart from it, that is allowed. For instance when the outcome is too harsh in a particular case. The receipt then says what was changed, why and by whom.',
    variantsLabel: 'Three ways to depart',
    variants: [
      {
        id: 'lex',
        label: 'The law itself departs',
        short: 'The law',
        rows: [
          ['What', 'the time limit for an objection'],
          ['Under the general rule', '6 weeks (General Administrative Law Act)'],
          ['Becomes', '4 weeks'],
          ['Why', 'the Vreemdelingenwet 2000 says so, in Article 69'],
        ],
        note: 'A specific law departs from the general one. That is already in the published rules.',
      },
      {
        id: 'orgaan',
        label: 'The mayor departs in one case',
        short: 'One case',
        rows: [
          ['What', 'how long a house has to be closed'],
          ['Under the rule', '6 months'],
          ['Becomes', '3 months'],
          ['Why', 'six months would be too hard on the children in the family'],
          ['Who', 'the mayor'],
        ],
        note: 'Someone looks at the case and departs from the rule. That is only allowed where the law gives room for it.',
      },
      {
        id: 'batch',
        label: 'A correction for a whole group',
        short: 'A group',
        rows: [
          ['What', 'the outcome the rules gave'],
          ['Becomes', 'what the court decided'],
          ['Why', 'a ruling about a whole group of decisions'],
          ['Who', 'the organization, for all decisions at once'],
        ],
        note: 'The receipt then also says that a program did this, and not a person who read the file.',
      },
    ],
    more: [
      'If such a departure happens automatically every time, for everyone with a certain feature, the published rule has in fact been replaced. The difference with today is that an auditor can find it, because it is on every receipt.',
      'The examples come from the paper. The house example is made up. The objection period in the Vreemdelingenwet is real.',
    ],
  },

  limits: {
    title: 'What this does not fix',
    intro: [
      'As long as people carried out the law, there was room to move that was written down nowhere. An official helped someone along although no rule allowed it, and nobody looked. That was not actually allowed, however good it was. But the case came out better than the rule would have made it.',
      'Software has no such room. It does what has been recorded and nothing else. That is already true of the systems the government uses today, and it has nothing to do with publishing: a rulework nobody can see is just as strict.',
      'The proposal does not bring that unwritten room back, and it cannot, because an exception you write down is simply another rule. It does give part of the room back, because the computer’s outcome is only a default. And it makes visible what is written down nowhere today:',
    ],
    explicit: [
      [
        'Where there is room',
        'If the law gives the authority room to depart, that is in the published rulework. So are the grounds the General Administrative Law Act gives for it.',
      ],
      ['Where there is none', 'If the law gives no such room, the rulework offers nothing to depart with either.'],
      [
        'That a departure happened',
        'Every departure is on the receipt: what the rules computed, what it became, on what ground and who did it. Also when a program did it rather than a person (step 5).',
      ],
    ],
    explicitNote: 'That way you can see how strict the rules are, and who made which choice.',
    p2: 'The paper calls its approach imperfect, and it leaves a number of questions open.',
    openLabel: 'Open questions',
    open: [
      'What kind of legal document a rulework actually is.',
      'Whether you have a right to the receipt, and what you may then see about others, such as your partner’s income.',
      'Whether lawyers can check the rules properly without help.',
      'How to be sure that two programs calculate the same thing.',
      'Who has to prove what when a decision does not match the published rules.',
    ],
    more: [
      'A familiar worry is that people will dodge rules once they know them. That is why the paper draws a line. Only the rules that work out a decision become public. How government chooses whom to check is not part of that. Anyone who can afford an adviser already knows the thresholds today.',
      'There was also room in enforcement. Not every rule was checked for everyone, because there was no time to. A system that keeps checking everyone automatically takes that room away. The paper adds that it belongs itself to the movement that makes that room smaller.',
      'A system you can check may be given more tasks. The paper says that too.',
    ],
  },

  veterans: {
    title: 'For those who have worked on rules as code for years',
    p1:
      'Writing laws down as rules for a computer is not new. Think of RegelSpraak at the Dutch Tax Administration, FLINT at the Dutch immigration service, OpenFisca and Catala in France and Better Rules in New Zealand. They help specialists write rules and run them.',
    p2:
      'The paper asks for something else. The published rules must be guaranteed to be the rules that make the decision, and whoever gets the decision must be able to check that. None of these systems set out to do that. That is why the paper adds that the comparison below is not a ranking.',
    tableLabel: 'Earlier approaches compared',
    rowHeader: 'Approach',
    columns: ['Runs', 'Public', 'Bound'],
    columnHelp: [
      'A computer can calculate with the rules (in the paper: executable).',
      'Anyone can read the rules (published).',
      'It is guaranteed that the published rules are the rules that make the decision (bound to execution). This is what the paper asks for.',
    ],
    tableNote:
      'That only this paper has three yeses is no coincidence. The last column is exactly what the paper is about, and the reason it was written. The other approaches were built to write rules and run them, and they are good at that. This proposal is about whoever wants to read and check the rules afterwards.',
    p3:
      'An example closer to home is the Dutch Environment and Planning Act (Omgevingswet). There, rules are online in machine-readable form, and they drive the permit check in the national portal. Still, the law, the rules in the portal and the municipality’s software are three separate things that people keep in line by hand. The proposal makes them one.',
    more: ['The paper does not choose a controlled natural language like RegelSpraak, and gives three reasons:'],
    reasons: [
      'Several programs have to calculate the same thing. Two programs can read text that looks like ordinary language differently, without either being demonstrably wrong.',
      'When a law changes, it must be exactly clear what changed. In ordinary language a different wording can look the same and still do something else.',
      'The problem the paper is about lies in reading, not writing. RegelSpraak is made for whoever writes the rule, this proposal for whoever wants to check it afterwards.',
    ],
    moreAfter: [
      'The paper does not rule out a layer in ordinary language that translates into this format. And it adds that it defends this choice with arguments, not with measurements.',
      'In the current software, language models translate laws into this format, article by article. They never calculate anything themselves. A decision is worked out by a fixed program, with rules that people have reviewed and that the organization has adopted.',
    ],
  },
};

export const content: Record<Lang, Content> = { nl, en };

/**
 * Who can do what, now and with the proposal (step 2). The cells are claims
 * too, and each is in the claims file.
 */
export function actors(lang: Lang): ActorRow[] {
  const n = (v: Cell['v'], note?: string): Cell => ({ v, note });
  const t = lang === 'nl';
  return [
    {
      actor: t ? 'Wie de wet uitvoert' : 'Whoever carries out the law',
      now: [n('part', t ? 'niet altijd met het hele overzicht' : 'not always with the full picture'), n('part'), n('part')],
      proposal: [n('yes'), n('yes'), n('yes')],
    },
    {
      actor: t ? 'Jij, over je eigen besluit' : 'You, about your own decision',
      now: [n('no'), n('no', t ? 'je ziet alleen de uitkomst' : 'you only see the outcome'), n('no')],
      proposal: [n('yes'), n('yes', t ? 'je eigen besluit' : 'your own decision'), n('no')],
    },
    {
      actor: t ? 'Bezwaar en rechter' : 'Objection and court',
      now: [n('part', t ? 'moet het zelf uitzoeken' : 'has to work it out alone'), n('no'), n('no')],
      proposal: [n('yes'), n('yes', t ? 'het besluit dat voorligt' : 'the decision in front of them'), n('no')],
    },
    {
      actor: t ? 'Controleurs met toegang tot de gegevens' : 'Auditors with access to the data',
      now: [n('no', t ? 'zien vooral beschrijvingen' : 'mostly see descriptions'), n('no'), n('no')],
      proposal: [n('yes'), n('yes'), n('yes', t ? 'de echte besluiten' : 'real cases')],
    },
    {
      actor: t ? 'De Tweede Kamer en iedereen' : 'Parliament and everyone else',
      now: [n('no'), n('no'), n('no')],
      proposal: [
        n('yes', t ? 'de hele regel' : 'the whole rule'),
        n('no', t ? 'geen gegevens van mensen' : 'no personal data'),
        n('yes', t ? 'op cijfers over de bevolking, zonder echte gevallen' : 'on figures, no real cases'),
      ],
    },
    {
      actor: t ? 'Wie een wet voorbereidt' : 'Whoever prepares a law',
      now: [n('part', t ? 'de wet wel, de software niet' : 'the law, not the software'), n('no'), n('no')],
      proposal: [
        n('yes'),
        n('no', t ? 'geen gegevens van mensen' : 'no personal data'),
        n('yes', t ? 'een voorstel, voordat het wet wordt' : 'a bill, before it is law'),
      ],
    },
  ];
}
