/*
 * The text of the plain-language page about the position paper "Rules as
 * Executed": /research/rules-as-executed/uitgelegd and /explained.
 *
 * Dutch is the source; the English is its translation, in the same register.
 * Edit both together (AGENTS.md treats a key in one dataset only as a bug, and
 * the docs-writing skill holds this file to the same rule as landing-content).
 *
 * Every claim here explains the paper as published on 31 August 2026 and has
 * to be traceable to it. The claim-by-claim mapping, with the sentence of the
 * paper that carries each one, is in paper-explained.claims.md next to this
 * file. A sentence that cannot be traced there does not belong here,
 * however true it may be about the code today: the page explains a frozen
 * paper, and the code has moved since.
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
  rows: [string, string][];
  note: string;
}

export interface Content {
  meta: { title: string; description: string };
  header: {
    title: string;
    subtitle: string;
    lede: string;
    version: (date: string) => string;
    engineNote: string;
    veterans: string;
    otherLang: string;
    tocLabel: string;
    pdf: string;
    paper: string;
    discuss: string;
  };
  readInPaper: string;
  step: string;
  thisPaper: string;
  tableLink: string;
  yes: string;
  part: string;
  no: string;
  now: string;
  proposal: string;

  published: {
    title: string;
    p1: string;
    caseIntro: string;
    caseLabel: string;
    options: { id: string; label: string; outcome: string; tag: string; tone: 'success' | 'critical' }[];
    caseNote: string;
    layers: { label: string; tag: string; text: string; hidden: boolean }[];
    p2: string;
    demandLabel: string;
    demand: string;
    p3: string;
  };
  power: {
    title: string;
    p1: string;
    p2: string;
    matrixLabel: string;
    columns: [string, string, string];
    p3: string;
    p4: string;
  };
  binding: {
    title: string;
    p1: string;
    flowLabel: string;
    flowNow: { label: string; text: string }[];
    flowNowNote: string;
    flowProposal: { label: string; text: string }[];
    flowProposalNote: string;
    p2: string;
    exampleLabel: string;
    exampleStatute: string;
    exampleNote: string;
    mapColumns: [string, string, string];
    mapRows: [string, string, string][];
    showListing: string;
    p3: string;
    p4: string;
  };
  receipt: {
    title: string;
    p1: string;
    p2: string;
    demoIntro: string;
    decision: string;
    fields: {
      law: string;
      version: string;
      digest: string;
      date: string;
      inputs: string;
      amount: string;
    };
    inputsSummary: (n: number) => string;
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
    disclaimer: string;
    p3: string;
    limitsLabel: string;
    limits: string[];
  };
  default: {
    title: string;
    p1: string;
    variantsLabel: string;
    variants: OverrideVariant[];
    p2: string;
    p3: string;
  };
  offices: {
    title: string;
    p1: string;
    p2: string;
    quote: string;
    quoteNote: string;
    p3: string;
  };
  diagnose: {
    title: string;
    p1: string;
    orderLabel: string;
    orderIntro: string;
    orderOptions: { id: string; label: string }[];
    steps: { register: string; check: string }[];
    consulted: string;
    skipped: string;
    stop: string;
    orderNote: string;
    p2: string;
    p3: string;
    landingLink: string;
    gapLabel: string;
    gapRows: [string, string][];
    gapNote: string;
    p4: string;
  };
  veterans: {
    title: string;
    p1: string;
    tableLabel: string;
    columns: [string, string, string, string];
    p2: string;
    p3: string;
    p4: string;
    reasons: string[];
    p5: string;
  };
  limits: {
    title: string;
    p1: string;
    p2: string;
    p3: string;
    openLabel: string;
    open: string[];
  };
}

const nl: Content = {
  meta: {
    title: 'Rules as Executed, uitgelegd · RegelRecht',
    description:
      'Het position paper Rules as Executed in gewone taal: waarom de uitvoering van wetten gepubliceerd moet worden, en hoe je een besluit dan zelf narekent.',
  },
  header: {
    title: 'Rules as Executed, uitgelegd',
    subtitle: 'Het position paper in gewone taal',
    lede:
      'De overheid voert wetten uit met software. De wet staat in het Staatsblad, de software die beslist staat nergens. Het paper stelt voor die uitvoering te publiceren in een vorm die juristen kunnen lezen en een computer kan draaien, en de overheid te verplichten met precies die gepubliceerde versie te beslissen. Deze pagina loopt het betoog in negen stappen door.',
    version: (date) =>
      `Uitleg bij het paper zoals het op ${date} verscheen. Het paper is leidend: waar deze pagina iets eenvoudiger zegt dan het paper, geldt wat daar staat.`,
    engineNote:
      'De rekenvoorbeelden draaien in je browser op de huidige referentie-implementatie. Die is na het paper verder ontwikkeld, dus een bedrag kan afwijken van wat er in augustus 2026 uitkwam.',
    veterans:
      'Werk je al jaren aan rules as code? Stap 8 zegt wat dit voorstel anders doet dan wat je kent.',
    otherLang: 'Read this in English',
    tocLabel: 'De negen stappen',
    pdf: 'Pdf op Zenodo',
    paper: 'Het volledige paper',
    discuss: 'Bespreek het paper',
  },
  readInPaper: 'Lees dit in het paper',
  step: 'Stap',
  thisPaper: 'Dit paper',
  tableLink: 'Tabel 1 in het paper, met het volledige onderschrift',
  yes: 'ja',
  part: 'deels',
  no: 'nee',
  now: 'Nu',
  proposal: 'Onder het voorstel',

  published: {
    title: 'Gepubliceerd is niet hetzelfde als uitgevoerd',
    p1:
      'Bij de zorgtoeslag telt het inkomen van je partner mee. Wie als partner geldt, staat in een andere wet. Dat is allemaal gepubliceerd. Maar een systeem moet meer weten dan de wet zegt: uit welke registratie het een partner afleidt, op welke datum het kijkt, wat het doet met iemand die halverwege het jaar gaat samenwonen. Die keuzes maakt de organisatie die de wet uitvoert. Soms legt ze die vast in een gepubliceerde beleidsregel. Vaker zitten ze in werkinstructies en in code, en dan kent niemand buiten de organisatie ze.',
    caseIntro: 'Een verzonnen geval: Sam gaat op 1 maart samenwonen en vraagt in juni zorgtoeslag aan.',
    caseLabel: 'Hoe stelt het systeem vast of Sam een partner heeft?',
    options: [
      {
        id: 'jan',
        label: 'Registratie op 1 januari',
        outcome: 'Geen partner. Sam krijgt zorgtoeslag op het eigen inkomen.',
        tag: 'toeslag',
        tone: 'success',
      },
      {
        id: 'aanvraag',
        label: 'Registratie op de dag van de aanvraag',
        outcome: 'Wel een partner. Het inkomen samen ligt boven de grens: geen zorgtoeslag.',
        tag: 'geen toeslag',
        tone: 'critical',
      },
    ],
    caseNote:
      'Beide keuzes zijn verzonnen. Ze laten zien waar zo’n keuze zit, niet hoe de Dienst Toeslagen het werkelijk doet. Dat kun je van buitenaf ook niet nagaan, en daar gaat het paper over.',
    layers: [
      { label: 'De wet', tag: 'Gepubliceerd', text: 'Het inkomen van een partner telt mee.', hidden: false },
      {
        label: 'De uitvoering',
        tag: 'Niet gepubliceerd',
        text: 'Welke registratie, welke peildatum, wat bij een verhuizing.',
        hidden: true,
      },
      { label: 'Het besluit', tag: 'Bij Sam thuis', text: 'Een bedrag, of nul.', hidden: false },
    ],
    p2:
      'Vaak overziet zelfs de organisatie die het systeem draait niet meer wat het doet: logica die over jaren is opgestapeld, soms in software van een leverancier wiens broncode de overheid niet mag inzien.',
    demandLabel: 'De vierde eis',
    demand:
      'De rechtsstaat vraagt dat regels kenbaar, begrijpelijk en voorspelbaar zijn. Digitale uitvoering voegt daar een eis aan toe: de regels zoals ze worden uitgevoerd, moeten onafhankelijk te controleren zijn.',
    p3:
      'Volgens het paper is dat geen nieuwe waarde. Zolang mensen de wet toepasten, zat die eis in de eerste drie besloten. Nu software de wet toepast, is het de voorwaarde waaronder die drie overeind blijven.',
  },

  power: {
    title: 'Een machtsvraag',
    p1:
      'Alleen wie de software draait, kan nagaan wat die doet. Dat geeft de uitvoerende macht een voorsprong op de wetgever en de rechter, en het paper behandelt het daarom als een kwestie van machtsevenwicht. De Kamer ziet niet hoe een wet die ze aannam in de praktijk werkt. De rechter krijgt de uitkomst te zien en moet de uitleg van de uitvoerder aannemen of de redenering zelf reconstrueren. De burger krijgt een besluit en kan niet nagaan of het uit de wet volgt.',
    p2:
      'Twee plichten uit de Grondwet lopen daar vast. Een minister is verantwoordelijk voor wat het ministerie doet (artikel 42) en geeft de Kamer de informatie waar die om vraagt (artikel 68). Voor een regel die niemand kan lezen, lukt geen van beide.',
    matrixLabel: 'Wie kan wat met de regel zoals die wordt uitgevoerd?',
    columns: ['Leest de regel', 'Rekent één geval na', 'Rekent een populatie na'],
    p3:
      'Om dat gat heen heeft de overheid vervangende waarborgen gebouwd: het Algoritmeregister, DPIA’s, het IAMA, proportionaliteitscommissies. Ze beoordelen allemaal beschrijvingen van systemen, opgesteld door wie die systemen bouwde. Met een gepubliceerde regel krijgen ze iets anders te beoordelen: de regel zelf.',
    p4:
      'Het paper noemt de toeslagenaffaire, Robodebt in Australië en het Horizon-schandaal in het Verenigd Koninkrijk als uitingen van hetzelfde patroon. Over de toeslagenaffaire zegt het dat ondoorzichtige uitvoering één draad van het falen was. De parlementaire enquête concludeerde dat alle drie de machten blind waren geweest voor mens en recht.',
  },

  binding: {
    title: 'Publiceer wat er draait',
    p1:
      'Het voorstel: publiceer de uitvoering als specificatie, en verplicht de organisatie om met die gepubliceerde specificatie te beslissen. De specificatie staat dan niet naast het systeem als beschrijving die hopelijk klopt. Het is het bestand dat de engine uitvoert. Publiceren en uitvoeren worden dezelfde handeling op hetzelfde bestand.',
    flowLabel: 'Van wet naar besluit',
    flowNow: [
      { label: 'Wettekst', text: 'Gepubliceerd in het Staatsblad' },
      { label: 'Beleidsregels en werkinstructies', text: 'Deels gepubliceerd' },
      { label: 'Code in het systeem', text: 'Niet gepubliceerd' },
      { label: 'Besluit', text: 'Zonder vastlegging van welke regel besliste' },
    ],
    flowNowNote:
      'Elke stap is een vertaling van de vorige, met de hand bijgehouden. Niets garandeert dat de laatste nog met de eerste overeenkomt.',
    flowProposal: [
      { label: 'Wettekst', text: 'Gepubliceerd in het Staatsblad' },
      { label: 'Specificatie', text: 'Gepubliceerd, en de enige versie die draait' },
      { label: 'Besluit', text: 'Met een vastlegging van de specificatie en de invoer' },
    ],
    flowProposalNote:
      'De uitleg die de organisatie toepast, staat op één plek, en die plek is openbaar. Wie het besluit krijgt, kan het naast de gepubliceerde specificatie leggen.',
    p2:
      'Het formaat is met opzet beperkt. Er zit weinig in: rekenen, vergelijken, nagaan of iets in een lijst staat, datums toetsen, en verwijzen naar andere regels. Geen lussen, geen recursie, geen algemene programmeertaal. Daardoor eindigt elke berekening, en blijft de specificatie leesbaar voor juristen. Ze volgt de artikelindeling van de wet, zodat je artikel voor artikel kunt nagaan of de vertaling klopt.',
    exampleLabel: 'Een verzonnen artikel en zijn specificatie (naar figuur 1 van het paper)',
    exampleStatute:
      'Een bewoner komt in aanmerking voor een parkeervergunning als zijn voertuig weinig uitstoot. Voor een emissievrij voertuig geldt een lager tarief.',
    exampleNote:
      'Elke zinsnede van het artikel heeft een eigen regel, en elke waarde verwijst naar de regeling waar ze vandaan komt. Er staat geen los getal in.',
    mapColumns: ['In de wet', 'In de specificatie', 'Waar de waarde vandaan komt'],
    mapRows: [
      ['Een bewoner', 'is_resident', 'de basisregistratie (civil_registry)'],
      ['als zijn voertuig weinig uitstoot', 'is_low_emission', 'het kentekenregister (vehicle_registry)'],
      ['komt in aanmerking voor een parkeervergunning', 'eligible = AND(is_resident, is_low_emission)', 'dit artikel rekent het uit'],
      [
        'Voor een emissievrij voertuig geldt een lager tarief',
        'permit_fee = IF is_zero_emission THEN reduced_fee ELSE standard_fee',
        'de tarieventabel (parking_fee_schedule)',
      ],
    ],
    showListing: 'De volledige specificatie, zoals figuur 1 van het paper haar afdrukt',
    p3:
      'De scherpste tegenwerping: zo verschuift de uitlegmacht alleen. Wie de specificatie schrijft, legt de wet uit, en heeft nu het gezag van publicatie erbij. Het paper erkent dat de uitvoerder blijft uitleggen en als eerste zet. Maar die uitleg gebeurde al, in code en werkinstructies die niemand kon zien. Publicatie maakt een stille keuze tot een openbare, waar iemand op aangesproken kan worden.',
    p4:
      'De specificatie is een uitleg van de wet, niet de wet zelf. Botst ze met de wettekst, dan gaat de wet voor en moet de specificatie worden verbeterd. Wat voor juridisch object zo’n specificatie is, laat het paper open; het dichtst in de buurt komt een wetsinterpreterende beleidsregel. Of juristen een specificatie zonder hulp betrouwbaar kunnen controleren, noemt het paper een hypothese die nog getoetst moet worden.',
  },

  receipt: {
    title: 'Het bonnetje',
    p1:
      'Onder het voorstel krijgt elk besluit een vastlegging mee: welke specificaties de berekening gebruikte, vastgelegd als vingerafdruk van hun inhoud, met welke invoer, met welke engine, en wat eruit kwam. De organisatie ondertekent dat geheel.',
    p2:
      'De vingerafdruk laat zien welke versie de organisatie zegt te hebben gebruikt. Dat die versie het besluit ook nam, blijkt pas als je narekent: draai de gepubliceerde regel op de vastgelegde invoer en kijk of dezelfde uitkomst eruit komt. Omdat de uitkomst alleen van de specificatie en de invoer afhangt, kan dat altijd, en komt er bij dezelfde invoer altijd hetzelfde uit.',
    demoIntro:
      'Hieronder twee besluiten over de zorgtoeslag van dezelfde persoon, met de testgegevens uit het corpus. Ze zijn zojuist in je browser berekend. Reken ze na.',
    decision: 'Besluit',
    fields: {
      law: 'Specificatie',
      version: 'Versie',
      digest: 'Vingerafdruk (SHA-256)',
      date: 'Rekendatum',
      inputs: 'Vastgelegde invoer',
      amount: 'Zorgtoeslag volgens het besluit',
    },
    inputsSummary: (n) => `${n} waarden uit de registraties, dezelfde voor beide besluiten`,
    check: 'Reken na',
    checking: 'Bezig met narekenen…',
    checkDigest: 'Vingerafdruk is die van de gepubliceerde versie',
    checkAmount: 'De gepubliceerde regel geeft hetzelfde bedrag',
    ok: 'klopt',
    mismatch: 'klopt niet',
    loading: 'De engine en de wetten worden opgehaald…',
    failed: 'Berekenen lukte niet. Probeer de pagina opnieuw te laden.',
    noScript: 'Hiervoor is JavaScript nodig: de engine draait in je eigen browser, niet op een server.',
    revealA:
      'Dit besluit is genomen met de gepubliceerde versie. Je narekening gebruikt dezelfde engine als het besluit; met een andere engine leun je erop dat beide hetzelfde uitrekenen, en hoe je dat vaststelt is een open vraag in het paper.',
    revealB: (published, local, diff) =>
      `Het systeem achter dit besluit draaide een eigen kopie van de wet, waarin één percentage nog op de waarde van vorig jaar stond (${local} in plaats van ${published}). Die kopie is nooit gepubliceerd. Het verschil is ${diff}, en het valt op twee manieren op: de vingerafdruk past niet bij de gepubliceerde versie, en narekenen geeft een ander bedrag.`,
    disclaimer:
      'Beide besluiten zijn echt berekend, met de engine en de wetten die deze site laadt; besluit B met een kopie die deze pagina zelf aanpast. Een echte vastlegging bevat de vingerafdruk van elke specificatie die de berekening raakte en is ondertekend. Hier staat alleen die van de Wet op de zorgtoeslag, en deze pagina tekent niets.',
    p3:
      'Een besluit kan ook een kloppende vingerafdruk hebben en bij narekenen toch iets anders opleveren. Het paper noemt dan twee verklaringen. Of de engine van wie narekent rekent anders dan die van de organisatie. Of het besluit volgde de gepubliceerde specificatie niet, en is daarom gebrekkig. Dat verschil is een feit dat iedereen met de invoer kan vaststellen, geen kwestie van mening.',
    limitsLabel: 'Wat het bonnetje niet doet',
    limits: [
      'Het legt de invoer vast, niet of die invoer klopt. Daarvoor moet elke invoer gekoppeld worden aan de bron die er gezaghebbend voor is, waar zo’n bron bestaat.',
      'Het paper gaat uit van een overheid die de wet wil uitvoeren en daarbij fouten kan maken: een versie draaien die ze niet publiceerde, een bepaling verkeerd vertalen, het overzicht kwijtraken. Tegen een overheid die bewust misleidt, bijvoorbeeld door voor één geval twee vastleggingen te tekenen, beschermt dit ontwerp niet. Daar zijn andere middelen voor nodig, en die stelt het paper niet voor.',
      'Narekenen kan alleen wie de vastlegging heeft. Of de ontvanger van een besluit daar nu al recht op heeft, is volgens het paper onzeker. Het voorstel is dat die haar krijgt, bij het besluit, zonder erom te hoeven vragen.',
      'Publicatie en vastlegging moeten wettelijke plichten worden, met gevolgen voor het besluit als ze ontbreken. Welke gevolgen, laat het paper aan juristen.',
    ],
  },

  default: {
    title: 'Een uitkomst is een standaard',
    p1:
      'Binding aan de gepubliceerde regel betekent niet dat de computer beslist. Wat de specificatie uitrekent, is een standaarduitkomst. Waar de wet een bevoegdheid geeft om af te wijken, kan dat. De vastlegging laat dan zien welke waarde vervangen is, door welke, op welke grond en door wie.',
    variantsLabel: 'Drie soorten afwijking, één vastlegging',
    variants: [
      {
        id: 'lex',
        label: 'De wetgever',
        rows: [
          ['Uitkomst', 'bezwaartermijn'],
          ['Volgens de algemene regel', '6 weken (art. 6:7 Awb)'],
          ['Vervangen door', '4 weken'],
          ['Grond', 'in afwijking van art. 6:7 Awb'],
          ['Vastgelegd door', 'Vreemdelingenwet 2000, art. 69'],
        ],
        note:
          'De bijzondere wet wijkt af van de algemene. Die afwijking staat in de gepubliceerde specificatie zelf, en de verklaring is haar eigen grond.',
      },
      {
        id: 'orgaan',
        label: 'Het bestuursorgaan',
        rows: [
          ['Uitkomst', 'duur van de sluiting'],
          ['Volgens de regel', '6 maanden'],
          ['Vervangen door', '3 maanden'],
          ['Grond', 'art. 4:84 Awb'],
          ['Motivering', 'De standaardsluiting zou onevenredig zijn, gezien de woonsituatie en de school van de kinderen.'],
          ['Door', 'Burgemeester van X'],
          ['Gemaakt door', 'een behandelaar die het dossier woog'],
        ],
        note:
          'Het orgaan wijkt in één geval af, met een grond en een motivering. De bevoegdheid komt uit de bepaling die de ruimte geeft. Waar een wet bindt en geen ruimte laat, valt er niets te vervangen.',
      },
      {
        id: 'batch',
        label: 'Een correctie in bulk',
        rows: [
          ['Uitkomst', 'zoals de regel die uitrekende'],
          ['Vervangen door', 'wat de uitspraak voorschrijft'],
          ['Grond', 'uitspraak van de rechter over deze groep besluiten'],
          ['Door', 'het bestuursorgaan'],
          ['Gemaakt door', 'een proces, in één run voor duizenden besluiten'],
        ],
        note:
          'Een afwijking hoeft niet met de hand. De vastlegging zegt dan wel dat een proces het deed en geen mens die het dossier zag. Een evenredigheidsweging in een batch van duizenden die als weging door een behandelaar is vastgelegd, spreekt zichzelf tegen.',
      },
    ],
    p2:
      'De voorbeelden komen uit het paper (figuur 3 en paragraaf 4.7). De sluitingszaak is verzonnen; de bezwaartermijn uit de Vreemdelingenwet niet.',
    p3:
      'Eén afwijking op eigen grond laat de regel als standaard staan. Gaan afwijkingen automatisch af in elk geval dat aan een voorwaarde voldoet, dan is de gepubliceerde regel in feite vervangen, door logica die weer niemand kan lezen. Het verschil met vroeger is dat het patroon nu in ondertekende vastleggingen staat, waar het te vinden is.',
  },

  offices: {
    title: 'Ieder in zijn eigen ambt',
    p1:
      'Dezelfde gepubliceerde specificatie dient iedereen, maar niet iedereen op dezelfde manier. Wie een besluit krijgt, kan het eigen geval narekenen, en vooraf uitproberen wat een verhuizing of een ander inkomen doet. In bezwaar en beroep kan de rechter het voorliggende geval opnieuw uitvoeren en vaststellen of een uitkomst uit de specificatie volgt of uit de wettekst zelf. Controleurs met een wettelijke grondslag voor de gegevens kunnen een hele populatie narekenen. De Kamer en het publiek hebben geen casusgegevens en kunnen geen besluit narekenen. Zij krijgen de regel zelf: elke drempel, en het volledige effect van een amendement voordat het wordt aangenomen.',
    p2:
      'Dat narekenen bij de betrokkenen ligt en niet bij iedereen, is volgens het paper juist goed:',
    quote:
      'a state in which anyone could re-run anyone’s case would have bought transparency by abolishing privacy.',
    quoteNote: 'Een staat waarin iedereen ieders geval kan narekenen, koopt transparantie met privacy.',
    p3:
      'Publiceren verandert de machtsverhouding alleen als de andere machten met het gepubliceerde kunnen werken. Het paper verwacht niet dat Kamerleden zelf specificaties lezen. Het wijst op Bureau Wetgeving en de Dienst Analyse en Onderzoek van de Tweede Kamer, die die capaciteit zouden kunnen krijgen, voor alle fracties gelijk. In Frankrijk bestaat zoiets al: LexImpact rekent bij de Assemblée nationale amendementen door met OpenFisca.',
  },

  diagnose: {
    title: 'Wat coderen blootlegt',
    p1:
      'Wie een wet uitvoerbaar maakt, moet vragen beantwoorden die op papier open kunnen blijven. Welke gegevens heeft deze regel echt nodig? Wat gebeurt er bij invoer die niemand voorzag? Op welke andere wetten leunt hij? Zodra de regels formeel zijn, kun je ze ook doorrekenen: op invoer waarvoor de wet geen uitkomst geeft, op wetten die elkaar tegenspreken, op bepalingen die geen enkel geval kan bereiken. Nu komen zulke gaten meestal pas aan het licht in een rechtszaak.',
    orderLabel: 'Welke registraties worden geraadpleegd?',
    orderIntro:
      'Het voorbeeld uit paragraaf 7.1 van het paper. Voor de zorgtoeslag moet je verzekerd zijn, en een zorgverzekering wordt opgeschort tijdens detentie. Je moet ook minstens achttien zijn. De aanvrager hier is een kind van drie.',
    orderOptions: [
      { id: 'age', label: 'Eerst de leeftijd' },
      { id: 'detention', label: 'Eerst detentie' },
    ],
    steps: [
      { register: 'Basisregistratie personen', check: 'Is de aanvrager 18 of ouder?' },
      { register: 'Detentiegegevens', check: 'Is de verzekering opgeschort wegens detentie?' },
    ],
    consulted: 'geraadpleegd',
    skipped: 'niet nodig',
    stop: 'Uitkomst: geen recht. Verder kijken hoeft niet.',
    orderNote:
      'Beide volgordes zijn wettelijk toegestaan. Alleen in de eerste komen gevoelige gegevens die er niet toe doen nooit in beeld. Systemen raadplegen vaak alles waar ze bij mogen, omdat niemand het pad ziet. In een specificatie ligt het pad vast en kun je het nalopen.',
    p2:
      'Een Memorie van Toelichting rekent vaak voorbeelden voor. Die kunnen dienen als test: de gepubliceerde specificatie rekent ze na, of niet. Het paper noemt ook de beperking: zulke voorbeelden worden niet bijgewerkt als de wet verandert of als bedragen worden geïndexeerd, dus ze testen na een paar jaar niet meer de regel die geldt.',
    p3: 'Op de voorpagina draait zo’n voorbeeld uit de Kamerstukken van de zorgtoeslag.',
    landingLink: 'Naar het voorbeeld op de voorpagina',
    gapLabel: 'Een gat, gepubliceerd in plaats van verstopt (naar figuur 5 van het paper)',
    gapRows: [
      ['Artikel', '12'],
      ['Wat de tekst zegt', 'naar boven afgerond op hele euro’s'],
      ['Waarom het niet past', 'het formaat kent geen bewerking om af te ronden'],
      ['Wat nodig zou zijn', 'bewerkingen om af te ronden (ROUND, CEIL, FLOOR)'],
      ['Status', 'nog niet vrijgegeven: de engine weigert besluiten die van dit artikel afhangen'],
    ],
    gapNote:
      'Soms kan het formaat een bepaling niet uitdrukken. Een vertaler, en zeker een taalmodel, grijpt dan naar iets dat meestal hetzelfde uitkomt: een afronding nabootsen met rekenwerk, een tabel uitschrijven als reeks voorwaarden. Dat levert een specificatie op die draait en er trouw uitziet, maar afwijkt van de wet. Het paper wil dat het gat zelf wordt gepubliceerd, als aantekening bij het artikel.',
    p4:
      'Bij een bepaling als ‘naar het oordeel van de minister’ valt niets te benaderen. Die uitkomst is niet aan het formaat om uit te rekenen.',
  },

  veterans: {
    title: 'Voor wie al jaren aan rules as code werkt',
    p1:
      'Uitvoerbare wetgeving is niet nieuw. TAXMAN formaliseerde in 1977 Amerikaanse belastingregels voor de herstructurering van ondernemingen, in 1986 werd de British Nationality Act een logisch programma, de Belastingdienst werkte vanaf 1999 aan POWER en later aan RegelSpraak, de IND aan FLINT, Frankrijk aan OpenFisca en Catala, Nieuw-Zeeland aan Better Rules. Tabel 1 van het paper zet ze naast elkaar.',
    tableLabel: 'Eerdere benaderingen (tabel 1 van het paper)',
    columns: ['Benadering', 'Uitvoerbaar', 'Gepubliceerd', 'Gebonden aan uitvoering'],
    p2:
      'Het paper zegt erbij dat dit geen ranglijst is. Elk van deze systemen doet goed waarvoor het gebouwd is: specialisten helpen regels te schrijven en te draaien. De rechterkolom vraagt iets dat geen van hen als doel had: is de gepubliceerde regel gegarandeerd de regel die draait? Catala kan eigenschappen van een codering bewijzen, en dat compilatie die behoudt, maar niet dat het systeem dat iemands zaak besliste de gepubliceerde versie draaide.',
    p3:
      'Het dichtstbijzijnde voorbeeld is Nederlands. Onder de Omgevingswet publiceren overheden machineleesbare regels via STOP en TPOD, en toepasbare regels sturen de vergunningcheck in het Omgevingsloket. Daar zat tien jaar werk in. Toch staat de juridische regel in de ene vorm, de toepasbare regel in een tweede, en de toepassing in het zaaksysteem in een derde, en die drie gelijk houden blijft handwerk. Het voorstel maakt er één specificatie van, die gepubliceerd wordt omdat het besluit erop draait.',
    p4:
      'Het paper kiest bewust niet voor een gecontroleerde natuurlijke taal zoals RegelSpraak. Het erkent dat die keuze bij de Belastingdienst al jaren werkt, en geeft drie redenen om het toch anders te doen:',
    reasons: [
      'Meerdere engines moeten hetzelfde uitrekenen. Bij een tekst die op proza lijkt, kunnen twee engines een zin verschillend ontleden zonder dat een van beide aantoonbaar fout zit.',
      'Bij een wetswijziging moeten rechter, burger en wetgever precies zien wat er veranderde. In natuurlijke taal kan een herformulering hetzelfde lijken en iets anders doen, of andersom.',
      'Het knelpunt zit bij het lezen, niet bij het schrijven. RegelSpraak is gemaakt voor wie de regel opstelt; dit voorstel voor wie na de uitvoering moet kunnen controleren.',
    ],
    p5:
      'In de referentie-implementatie vertalen taalmodellen wetten naar het formaat, artikel voor artikel, volgens een gepubliceerde werkwijze. Ze rekenen nooit. Wat een besluit uitrekent, is de deterministische engine, op een specificatie die mensen hebben nagekeken en een organisatie heeft overgenomen.',
  },

  limits: {
    title: 'Wat dit niet oplost',
    p1:
      'Een ambtenaar mag alleen doen waarvoor de wet een bevoegdheid geeft. In een organisatie van mensen is die grens niet scherp: er gebeurt weleens iets behulpzaams waarvoor niemand bevoegd is, en niemand controleert het. Een samenleving leunt daar misschien meer op dan ze kan zeggen. Een systeem kent die ongeschreven marge niet. Het paper wijst erop dat publicatie daar niets aan verandert: de marge sluit net zo goed in software die nooit gepubliceerd wordt, en dat gebeurt nu al.',
    p2:
      'Een bekend bezwaar is dat wie de regel kent, hem kan ontduiken. Het paper scheidt daarom de beslisregel van de controlelogica. Alleen de eerste hoeft openbaar: hoe de wet op feiten tot een uitkomst komt. Hoe de overheid kiest wie ze controleert, valt erbuiten. Wie zijn zaken net onder een drempel regelt, kan dat omdat de wetgever een precieze drempel koos. Met een adviseur lukt dat nu ook al; geheimhouding benadeelt alleen wie geen adviseur kan betalen.',
    p3:
      'Het paper noemt zichzelf onvolmaakt, en zegt dat het nieuwe problemen introduceert. Het richt zich op de oorzaak die het aanwijst: dat de uitvoering van wetten ondoorzichtig is. Een deel van de vragen laat het uitdrukkelijk open.',
    openLabel: 'Open vragen uit het paper',
    open: [
      'Wat voor juridisch object een specificatie is, en of een correctie alleen voor de toekomst werkt.',
      'Of de ontvanger van een besluit recht heeft op de vastlegging, en wat die mag zien van gegevens over anderen, zoals het inkomen van een partner.',
      'Of juristen een specificatie zonder hulp betrouwbaar kunnen controleren.',
      'Hoe je vaststelt dat twee engines hetzelfde uitrekenen.',
      'Hoe de bewijslast wordt verdeeld als vaststaat dat een besluit afwijkt van de gepubliceerde specificatie.',
    ],
  },
};

const en: Content = {
  meta: {
    title: 'Rules as Executed, explained · RegelRecht',
    description:
      'The position paper Rules as Executed in plain language: why the execution of law should be published, and how you would then check a decision yourself.',
  },
  header: {
    title: 'Rules as Executed, explained',
    subtitle: 'The position paper in plain language',
    lede:
      'Government executes law with software. The law is published in the Staatsblad; the software that decides is published nowhere. The paper proposes publishing that execution in a form lawyers can read and a computer can run, and obliging government to decide with exactly the version it published. This page walks through the argument in nine steps.',
    version: (date) =>
      `An explanation of the paper as published on ${date}. The paper governs: where this page says something more simply than the paper does, the paper is what counts.`,
    engineNote:
      'The worked examples run in your browser on the current reference implementation. It has moved on since the paper, so an amount can differ from what came out in August 2026.',
    veterans:
      'Been working on rules as code for years? Step 8 says what this proposal does differently from what you know.',
    otherLang: 'Lees dit in het Nederlands',
    tocLabel: 'The nine steps',
    pdf: 'PDF on Zenodo',
    paper: 'The full paper',
    discuss: 'Discuss the paper',
  },
  readInPaper: 'Read this in the paper',
  step: 'Step',
  thisPaper: 'This paper',
  tableLink: 'Table 1 in the paper, with its full caption',
  yes: 'yes',
  part: 'partly',
  no: 'no',
  now: 'Now',
  proposal: 'Under the proposal',

  published: {
    title: 'Published is not the same as executed',
    p1:
      'For the Dutch healthcare allowance, your partner’s income counts. Who counts as a partner is set out in another act. All of that is published. But a system needs to know more than the law says: which register it derives a partner from, on which date it looks, what it does with someone who moves in with a partner halfway through the year. The organization that executes the law makes those choices. Sometimes it records them in a published policy rule (beleidsregel). More often they live in work instructions and in code, and then nobody outside the organization knows them.',
    caseIntro: 'A made-up case: Sam moves in with a partner on 1 March and applies for the allowance in June.',
    caseLabel: 'How does the system decide whether Sam has a partner?',
    options: [
      {
        id: 'jan',
        label: 'Register on 1 January',
        outcome: 'No partner. Sam receives the allowance on their own income.',
        tag: 'allowance',
        tone: 'success',
      },
      {
        id: 'aanvraag',
        label: 'Register on the day of application',
        outcome: 'A partner. Their combined income is above the limit: no allowance.',
        tag: 'no allowance',
        tone: 'critical',
      },
    ],
    caseNote:
      'Both choices are made up. They show where such a choice sits, not how the Dutch benefits agency actually does it. From the outside you cannot check that either, and that is what the paper is about.',
    layers: [
      { label: 'The law', tag: 'Published', text: 'A partner’s income counts.', hidden: false },
      {
        label: 'The execution',
        tag: 'Not published',
        text: 'Which register, which reference date, what happens on a move.',
        hidden: true,
      },
      { label: 'The decision', tag: 'In Sam’s letterbox', text: 'An amount, or zero.', hidden: false },
    ],
    p2:
      'Often even the organization running the system no longer has an overview of what it does: logic layered up over years, sometimes in software from a supplier whose source code the government may not inspect.',
    demandLabel: 'The fourth demand',
    demand:
      'The rule of law asks that rules be knowable, comprehensible and predictable. Digital execution adds a demand: the rules as executed must be independently verifiable.',
    p3:
      'According to the paper this is not a new value. As long as people applied the law, the demand was implicit in the other three. Now that software applies it, it is the condition under which those three survive.',
  },

  power: {
    title: 'A question of power',
    p1:
      'Only whoever runs the software can find out what it does. That gives the executive an advantage over the legislature and the courts, which is why the paper treats it as a question of the balance of powers. Parliament does not see how a law it passed works in practice. A court sees the outcome and has to accept the executive’s account or reconstruct the reasoning itself. A citizen receives a decision and cannot check whether it follows from the law.',
    p2:
      'Two constitutional duties stall there. A minister is responsible for what the ministry does (Article 42 of the Dutch Constitution) and gives Parliament the information it asks for (Article 68). For a rule nobody can read, neither works.',
    matrixLabel: 'Who can do what with the rule as executed?',
    columns: ['Reads the rule', 'Re-runs one case', 'Re-runs a population'],
    p3:
      'Around that gap the Dutch state has built substitute safeguards: the Algorithm Register, data protection impact assessments, the human-rights impact assessment IAMA, proportionality committees. All of them assess descriptions of systems, written by the people who built those systems. With a published rule they get something else to assess: the rule itself.',
    p4:
      'The paper names the Dutch childcare benefits scandal (toeslagenaffaire), Robodebt in Australia and the Post Office Horizon scandal in the United Kingdom as instances of the same pattern. Of the toeslagenaffaire it says that opaque execution was one strand of the failure. The parliamentary inquiry concluded that all three branches had been blind to people and justice.',
  },

  binding: {
    title: 'Publish what runs',
    p1:
      'The proposal: publish execution as a specification, and oblige the organization to decide with that published specification. The specification then does not sit beside the system as a description that is hoped to agree with it. It is the file the engine executes. Publishing and executing become the same act on the same file.',
    flowLabel: 'From law to decision',
    flowNow: [
      { label: 'Statute', text: 'Published in the Staatsblad' },
      { label: 'Policy rules and work instructions', text: 'Partly published' },
      { label: 'Code in the system', text: 'Not published' },
      { label: 'Decision', text: 'With no record of which rule decided' },
    ],
    flowNowNote:
      'Each step translates the one before, kept in line by hand. Nothing guarantees that the last still matches the first.',
    flowProposal: [
      { label: 'Statute', text: 'Published in the Staatsblad' },
      { label: 'Specification', text: 'Published, and the only version that runs' },
      { label: 'Decision', text: 'With a record of the specification and the inputs' },
    ],
    flowProposalNote:
      'The reading the organization applies sits in one place, and that place is public. Whoever receives the decision can hold it up against the published specification.',
    p2:
      'The format is restricted on purpose. There is little in it: arithmetic, comparison, checking whether something is in a list, date tests, and references to other rules. No loops, no recursion, no general-purpose programming language. That way every calculation ends, and the specification stays readable for lawyers. It follows the article structure of the statute, so you can check article by article whether the translation holds.',
    exampleLabel: 'A made-up article and its specification (after Figure 1 of the paper)',
    exampleStatute:
      'A resident is eligible for a parking permit if their vehicle has low emissions. The fee is reduced for zero-emission vehicles.',
    exampleNote:
      'Each phrase of the article gets a rule of its own, and every value points to the regulation it comes from. No bare number appears.',
    mapColumns: ['In the statute', 'In the specification', 'Where the value comes from'],
    mapRows: [
      ['A resident', 'is_resident', 'the civil registry (civil_registry)'],
      ['if their vehicle has low emissions', 'is_low_emission', 'the vehicle registry (vehicle_registry)'],
      ['is eligible for a parking permit', 'eligible = AND(is_resident, is_low_emission)', 'computed by this article'],
      [
        'The fee is reduced for zero-emission vehicles',
        'permit_fee = IF is_zero_emission THEN reduced_fee ELSE standard_fee',
        'the fee schedule (parking_fee_schedule)',
      ],
    ],
    showListing: 'The full specification, as Figure 1 of the paper prints it',
    p3:
      'The sharpest objection: this only relocates the power to interpret. Whoever writes the specification interprets the law, now with the authority of publication added. The paper grants that the executive keeps interpreting and moves first. But that interpretation was already happening, in code and work instructions nobody could see. Publication turns a silent choice into a public one that someone can be held to.',
    p4:
      'The specification is an interpretation of the law, not the law itself. Where it conflicts with the statute, the statute prevails and the specification has to be corrected. What kind of legal object such a specification is, the paper leaves open; the nearest existing category is a policy rule that interprets a statute (wetsinterpreterende beleidsregel). Whether lawyers can reliably verify a specification unaided, the paper calls a hypothesis still to be tested.',
  },

  receipt: {
    title: 'The receipt',
    p1:
      'Under the proposal every decision comes with a record: which specifications the calculation used, recorded as a fingerprint of their content, with which inputs, with which engine, and what came out. The organization signs the whole.',
    p2:
      'The fingerprint shows which version the organization says it used. That this version also made the decision only shows when you re-run it: run the published rule on the recorded inputs and see whether the same outcome comes out. Because the outcome depends only on the specification and the inputs, that is always possible, and the same inputs always give the same result.',
    demoIntro:
      'Below are two decisions on the healthcare allowance of the same person, using the test data from the corpus. They were just computed in your browser. Check them.',
    decision: 'Decision',
    fields: {
      law: 'Specification',
      version: 'Version',
      digest: 'Fingerprint (SHA-256)',
      date: 'Calculation date',
      inputs: 'Recorded inputs',
      amount: 'Allowance according to the decision',
    },
    inputsSummary: (n) => `${n} values from the registers, the same for both decisions`,
    check: 'Check it',
    checking: 'Checking…',
    checkDigest: 'The fingerprint is that of the published version',
    checkAmount: 'The published rule gives the same amount',
    ok: 'matches',
    mismatch: 'does not match',
    loading: 'Fetching the engine and the laws…',
    failed: 'The calculation failed. Try reloading the page.',
    noScript: 'This needs JavaScript: the engine runs in your own browser, not on a server.',
    revealA:
      'This decision was made with the published version. Your check uses the same engine as the decision; with a different engine you rely on both computing the same thing, and how to establish that is an open question in the paper.',
    revealB: (published, local, diff) =>
      `The system behind this decision ran its own copy of the law, in which one percentage still had last year’s value (${local} instead of ${published}). That copy was never published. The difference is ${diff}, and it shows in two ways: the fingerprint does not match the published version, and re-running gives a different amount.`,
    disclaimer:
      'Both decisions were really computed, with the engine and the laws this site loads; decision B with a copy this page alters itself. A real record carries the fingerprint of every specification the calculation touched and is signed. Here only the one for the Wet op de zorgtoeslag is shown, and this page signs nothing.',
    p3:
      'A decision can also carry a matching fingerprint and still give something else when re-run. The paper then names two explanations. Either the verifier’s engine computes differently from the organization’s. Or the decision did not follow the published specification, and is defective on that ground. The divergence is a fact anyone holding the inputs can establish, not a matter of opinion.',
    limitsLabel: 'What the receipt does not do',
    limits: [
      'It fixes the inputs, not whether they are true. For that, each input has to be bound to the source that is authoritative for it, where such a source exists.',
      'The paper assumes a government that intends to execute the law and can fail at it: run a version it did not publish, translate a provision wrongly, lose track. Against a government that deliberately deceives, for instance by signing two records for one case, this design offers no protection. That takes other means, and the paper does not propose them.',
      'Only whoever holds the record can re-run it. Whether the recipient of a decision is entitled to it today is, according to the paper, unsettled. The proposal is that they receive it, with the decision, without having to ask.',
      'Publication and the record have to become statutory duties, with consequences for the decision when they are missing. Which consequences, the paper leaves to lawyers.',
    ],
  },

  default: {
    title: 'An outcome is a default',
    p1:
      'Being bound to the published rule does not mean the computer decides. What the specification computes is a default outcome. Where the law confers a power to depart from it, that is possible. The record then shows which value was replaced, by what, on what ground and by whom.',
    variantsLabel: 'Three kinds of departure, one record',
    variants: [
      {
        id: 'lex',
        label: 'The legislature',
        rows: [
          ['Outcome', 'objection period'],
          ['Under the general rule', '6 weeks (Art. 6:7 Awb)'],
          ['Replaced by', '4 weeks'],
          ['Ground', 'in derogation of Art. 6:7 Awb'],
          ['Declared by', 'Vreemdelingenwet 2000, Art. 69'],
        ],
        note:
          'The specific act departs from the general one. That departure sits in the published specification itself, and the declaration is its own ground.',
      },
      {
        id: 'orgaan',
        label: 'The competent authority',
        rows: [
          ['Outcome', 'closure duration'],
          ['Under the rule', '6 months'],
          ['Replaced by', '3 months'],
          ['Ground', 'Art. 4:84 Awb'],
          ['Reasons', 'The standard closure would be disproportionate given the children’s housing stability and schooling.'],
          ['By', 'Burgemeester van X'],
          ['Made by', 'a caseworker who weighed the file'],
        ],
        note:
          'The authority departs in one case, with a ground and reasons. The power comes from the provision that grants the room. Where a statute binds and leaves none, there is nothing to replace.',
      },
      {
        id: 'batch',
        label: 'A correction in bulk',
        rows: [
          ['Outcome', 'as the rule computed it'],
          ['Replaced by', 'what the ruling requires'],
          ['Ground', 'a court ruling on this group of decisions'],
          ['By', 'the competent authority'],
          ['Made by', 'a process, in one run for thousands of decisions'],
        ],
        note:
          'A departure need not be made by hand. The record does then say that a process made it and not a person who saw the file. A proportionality assessment that arrives in a batch of thousands yet is recorded as a caseworker’s weighing refutes itself.',
      },
    ],
    p2:
      'The examples come from the paper (Figure 3 and Section 4.7). The closure case is made up; the objection period in the Vreemdelingenwet is not.',
    p3:
      'One departure on its own ground leaves the rule standing as the default. When departures fire automatically in every case that meets some condition, the published rule has in practice been replaced, by logic nobody can read again. The difference with before is that the pattern now shows in signed records, where it can be found.',
  },

  offices: {
    title: 'Each within its own office',
    p1:
      'The same published specification serves everyone, but not everyone in the same way. Whoever receives a decision can re-run their own case, and try out in advance what a move or a different income would do. In objection and appeal the court can re-run the case before it and establish whether an outcome follows from the specification or from the statute itself. Auditors with a legal basis for the data can re-run a whole population. Parliament and the public hold no case data and cannot re-run a decision. They get the rule itself: every threshold, and the full effect of an amendment before it is passed.',
    p2:
      'That re-running falls to those with standing, and not to everyone, is according to the paper the right model:',
    quote:
      'a state in which anyone could re-run anyone’s case would have bought transparency by abolishing privacy.',
    quoteNote: '',
    p3:
      'Publication changes the balance of power only if the other branches can work with what is published. The paper does not expect members of parliament to read specifications themselves. It points to the House’s legislative drafting office (Bureau Wetgeving) and its Analysis and Research Service, which could be given that capacity, equally for every parliamentary group. France already has something like it: LexImpact, at the Assemblée nationale, runs deputies’ amendments through OpenFisca.',
  },

  diagnose: {
    title: 'What encoding exposes',
    p1:
      'Making a law executable forces answers to questions that can stay open on paper. What data does this rule actually need? What happens with input nobody foresaw? Which other laws does it lean on? Once the rules are formal, you can also analyze them: for inputs where the law gives no outcome, for laws that contradict each other, for provisions no case can reach. Today such gaps mostly come to light in court.',
    orderLabel: 'Which registers are consulted?',
    orderIntro:
      'The example from Section 7.1 of the paper. For the healthcare allowance you have to be insured, and health insurance is suspended during detention. You also have to be at least eighteen. The applicant here is a three-year-old.',
    orderOptions: [
      { id: 'age', label: 'Age first' },
      { id: 'detention', label: 'Detention first' },
    ],
    steps: [
      { register: 'Population register (BRP)', check: 'Is the applicant 18 or older?' },
      { register: 'Detention records', check: 'Is the insurance suspended because of detention?' },
    ],
    consulted: 'consulted',
    skipped: 'not needed',
    stop: 'Outcome: not eligible. Nothing more to look up.',
    orderNote:
      'Both orders are lawful. Only in the first do sensitive data that do not matter never come into view. Systems often consult everything they are allowed to, because nobody sees the path. In a specification the path is fixed and can be checked.',
    p2:
      'An explanatory memorandum (Memorie van Toelichting) often works through examples. Those can serve as a test: the published specification reproduces them, or it does not. The paper also names the limit: such examples are not updated when the law changes or amounts are indexed, so after a few years they no longer test the rule in force.',
    p3: 'The home page runs one such example from the parliamentary papers on the healthcare allowance.',
    landingLink: 'To the example on the home page',
    gapLabel: 'A gap, published rather than hidden (after Figure 5 of the paper)',
    gapRows: [
      ['Article', '12'],
      ['What the text says', 'naar boven afgerond op hele euro’s (rounded up to whole euros)'],
      ['Why it does not fit', 'the format has no rounding operation'],
      ['What it would need', 'rounding operations (ROUND, CEIL, FLOOR)'],
      ['Status', 'not yet cleared: the engine refuses decisions that depend on this article'],
    ],
    gapNote:
      'Sometimes the format cannot express a provision. A translator, and a language model in particular, will then reach for something that mostly comes out the same: imitate the rounding with arithmetic, write a table out as a chain of conditions. That yields a specification that runs and looks faithful, while it diverges from the law. The paper wants the gap itself published, as an annotation on the article.',
    p4:
      'For a provision like “at the discretion of the minister” there is nothing to approximate. That outcome is not the format’s to compute.',
  },

  veterans: {
    title: 'For those who have worked on rules as code for years',
    p1:
      'Executable law is not new. TAXMAN formalized US corporate reorganization tax law in 1977, the British Nationality Act became a logic program in 1986, the Dutch Tax Administration worked on POWER from 1999 and later on RegelSpraak, the Dutch immigration service on FLINT, France on OpenFisca and Catala, New Zealand on Better Rules. Table 1 of the paper sets them side by side.',
    tableLabel: 'Prior approaches (Table 1 of the paper)',
    columns: ['Approach', 'Executable', 'Published', 'Bound to execution'],
    p2:
      'The paper adds that this is not a scorecard. Each of these systems does well what it was built for: helping specialists author and run rules. The right-hand column asks something none of them set out to provide: is the published rule guaranteed to be the rule that runs? Catala can prove properties of an encoding, and that compilation preserves them, but not that the system deciding someone’s case ran the published version.',
    p3:
      'The closest precedent is Dutch. Under the Omgevingswet (Environment and Planning Act), authorities publish machine-readable rules through STOP and TPOD, and toepasbare regels drive the permit check in the national portal. That took a decade of work. Still, the legal rule sits in one form, the toepasbare regel in a second, and its application in the case system in a third, and keeping those three aligned remains handwork. The proposal makes them one specification, published because the decision runs on it.',
    p4:
      'The paper deliberately does not choose a controlled natural language such as RegelSpraak. It grants that the choice has worked at the Tax Administration for years, and gives three reasons to do it differently anyway:',
    reasons: [
      'Several engines have to compute the same thing. With text that reads like prose, two engines can parse a sentence differently without either being demonstrably wrong.',
      'When the law is amended, courts, citizens and the legislature need to see exactly what changed. In natural language a rewording can look the same and do something else, or the other way round.',
      'The bottleneck is reading, not writing. RegelSpraak is made for whoever authors the rule; this proposal for whoever has to check after execution.',
    ],
    p5:
      'In the reference implementation, language models translate laws into the format, article by article, following a published procedure. They never compute. What computes a decision is the deterministic engine, running a specification that people have reviewed and an organization has adopted.',
  },

  limits: {
    title: 'What this does not fix',
    p1:
      'An official may only do what the law gives them the power to do. In an organization of people that boundary is not sharp: something helpful happens for which nobody holds a power, and nobody checks. A society may depend on that more than it can say. A system has no such unwritten margin. The paper points out that publication changes nothing here: the margin closes just as much in software that is never published, and that is already happening.',
    p2:
      'A familiar objection is that whoever knows the rule can game it. So the paper separates the decision rule from the enforcement logic. Only the first has to be public: how the law turns facts into an outcome. How government chooses whom to check falls outside it. Whoever arranges their affairs just below a threshold can do so because the legislature chose a precise threshold. With an advisor that already works today; secrecy only disadvantages those who cannot afford one.',
    p3:
      'The paper calls itself imperfect, and says it introduces new problems. It aims at the cause it identifies: that the execution of law is opaque. Some questions it leaves explicitly open.',
    openLabel: 'Open questions from the paper',
    open: [
      'What kind of legal object a specification is, and whether a correction only works prospectively.',
      'Whether the recipient of a decision is entitled to the record, and what they may see of data about others, such as a partner’s income.',
      'Whether lawyers can reliably verify a specification unaided.',
      'How to establish that two engines compute the same thing.',
      'How the burden of proof is divided once a decision has been shown to diverge from the published specification.',
    ],
  },
};

export const content: Record<Lang, Content> = { nl, en };

/**
 * The actor matrix, now and under the proposal. Kept apart from the prose
 * because the cells are claims too, and each is in the claims file.
 */
export function actors(lang: Lang): ActorRow[] {
  const n = (v: Cell['v'], note?: string): Cell => ({ v, note });
  const t = lang === 'nl';
  return [
    {
      actor: t ? 'Uitvoerder' : 'Executing organization',
      now: [
        n('part', t ? 'vaak zonder overzicht' : 'often without an overview'),
        n('part'),
        n('part'),
      ],
      proposal: [n('yes'), n('yes'), n('yes')],
    },
    {
      actor: t ? 'Wie het besluit krijgt' : 'Recipient of a decision',
      now: [n('no'), n('no', t ? 'ziet alleen de uitkomst' : 'sees only the outcome'), n('no')],
      proposal: [n('yes'), n('yes', t ? 'het eigen geval' : 'their own case'), n('no')],
    },
    {
      actor: t ? 'Bezwaar en rechter' : 'Objection officer and court',
      now: [n('no'), n('no', t ? 'ziet alleen de uitkomst' : 'sees only the outcome'), n('no')],
      proposal: [n('yes'), n('yes', t ? 'het voorliggende geval' : 'the case before it'), n('no')],
    },
    {
      actor: t ? 'Controleurs met wettelijke toegang' : 'Auditors with lawful access',
      now: [
        n('no', t ? 'beoordelen beschrijvingen' : 'assess descriptions'),
        n('no'),
        n('no'),
      ],
      proposal: [n('yes'), n('yes'), n('yes')],
    },
    {
      actor: t ? 'Kamer en publiek' : 'Parliament and the public',
      now: [n('no'), n('no'), n('no')],
      proposal: [n('yes', t ? 'de hele regel' : 'the whole rule'), n('no', t ? 'geen casusgegevens' : 'no case data'), n('no')],
    },
  ];
}
