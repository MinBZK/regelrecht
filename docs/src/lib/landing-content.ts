/*
 * Bilingual landing content. One layout, two datasets.
 * Language is derived from the route path ("/en/..." → en, otherwise nl).
 *
 * The NL strings are the originals from the static landing page.
 * The EN strings are a translation kept in the same government register.
 */

export interface NavLink {
  label: string
  href: string
}

export interface LandingContent {
  meta: { title: string; description: string }
  nav: {
    brandTagline: string
    what: string
    how: string
    tools: string
    example: string
    faq: string
    signup: string
    docs: string
  }
  hero: { titleSmall: string; intro: string; cta: string }
  partners: { label: string; items: NavLink[] }
  whatIsIt: { title: string; lede: string; cards: { h: string; p: string }[] }
  whyImportant: {
    title: string
    lede: string
    problemSolutions: {
      problemTitle: string
      problemText: string
      solutionTitle: string
      solutionText: string
    }[]
  }
  howItWorks: { title: string; lede: string; steps: { title: string; text: string }[] }
  tools: {
    title: string
    items: { title: string; text: string; link?: NavLink; meta?: string }[]
  }
  example: {
    title: string
    lede: string
    cases: {
      img: string
      alt: string
      caption: string
      h: string
      p: string
      bullets: string[]
      reverse?: boolean
    }[]
  }
  innovation: {
    title: string
    ledeBefore: string
    ledeLink: NavLink
    ledeAfter: string
    cards: { meta: string; h: string; p: string }[]
  }
  references: {
    title: string
    lede: string
    items: {
      title: string
      meta: string
      text: string
      href: string
      linkLabel: string
    }[]
  }
  faq: { title: string; items: { q: string; a: string }[] }
  feedback: { title: string; body: string; cta: string; ctaHref: string }
  footer: {
    blurb: string
    linksTitle: string
    contactTitle: string
    partOfTitle: string
    copyright: string
    links: NavLink[]
    partOf: string[]
  }
  signup: {
    pageTitle: string
    lede: string
    noscript: string
    legend: string
    radioYes: string
    radioNo: string
    updates: string
    emailLabel: string
    nameLabel: string
    orgLabel: string
    orgPlaceholder: string
    roleLabel: string
    rolePlaceholder: string
    required: string
    requiredSr: string
    submit: string
    submitting: string
    companyHoneypot: string
    errEmailEmpty: string
    errEmailInvalid: string
    errName: string
    successTitle: string
    successBody: string
    successReset: string
    errorTitle: string
    errorBody: string
    errorReset: string
  }
}

const SIGNUP_NL_PATH = '/aanmelden'
const SIGNUP_EN_PATH = '/en/signup'
const GUIDE_PATH = '/guide/what-is-regelrecht'
const GITHUB = 'https://github.com/MinBZK/regelrecht'

export const content: Record<'nl' | 'en', LandingContent> = {
  nl: {
    meta: {
      title: 'RegelRecht — van wet naar digitale werking',
      description:
        'Een verkenning van het Ministerie van BZK naar transparante, machine-uitvoerbare wetgeving.',
    },
    nav: {
      brandTagline: 'Een verkenning van het Ministerie van BZK',
      what: 'Wat',
      how: 'Hoe',
      tools: 'Ecosysteem',
      example: 'Voorbeelden',
      faq: 'Vragen',
      signup: 'Aanmelden',
      docs: 'Documentatie',
    },
    hero: {
      titleSmall: 'van wet naar digitale werking',
      intro:
        'RegelRecht verkent hoe we transparante, eenduidige en consistente uitvoering van wetgeving kunnen realiseren. Een verkenning naar digitale mogelijkheden waarbij iedereen kan begrijpen hoe besluiten tot stand komen.',
      cta: 'Verken de mogelijkheden',
    },
    partners: {
      label: 'Een initiatief van',
      items: [
        {
          label: 'Ministerie van Binnenlandse Zaken en Koninkrijksrelaties',
          href: 'https://www.rijksoverheid.nl/ministeries/ministerie-van-binnenlandse-zaken-en-koninkrijksrelaties',
        },
        { label: 'Bureau Architectuur', href: 'https://minbzk.github.io/BASE/' },
        { label: 'Digilab', href: 'https://digilab.overheid.nl/' },
      ],
    },
    whatIsIt: {
      title: 'Wat is RegelRecht?',
      lede: 'De uitvoering van wetgeving kent verschillende uitdagingen: verschillende interpretaties, ondoorzichtige systemen en complex programmeerwerk dat vaak ver af staat van de oorspronkelijke wet. RegelRecht verkent of machine-uitvoerbare wetgeving een antwoord kan bieden — wetten die direct als uitvoerbare code geschreven worden, zonder tussenkomst van programmeurs.',
      cards: [
        {
          h: 'Van analoog recht naar code',
          p: 'Kunnen we traditionele wetgeving transformeren naar machine-uitvoerbare specificaties? We onderzoeken of dit de kloof tussen wetgever en uitvoering kan verkleinen.',
        },
        {
          h: 'Eén bron van waarheid',
          p: 'Wat als er één centrale, machine-uitvoerbare versie van elke wet bestaat die alle partijen gebruiken? We verkennen of dit interpretatieverschillen kan verminderen.',
        },
        {
          h: 'Volledige transparantie',
          p: 'Hoe maken we overheidsbesluiten inzichtelijker? We experimenteren met manieren waarop burgers kunnen zien welke regels gelden en hoe beslissingen tot stand komen.',
        },
      ],
    },
    whyImportant: {
      title: 'Waarom deze verkenning?',
      lede: 'De huidige manier waarop wetten worden toegepast kent verschillende uitdagingen in onze rechtsstaat. We onderzoeken of nieuwe technische benaderingen kunnen bijdragen aan oplossingen voor deze structurele vraagstukken.',
      problemSolutions: [
        {
          problemTitle: 'Verschillende interpretaties',
          problemText:
            'Dezelfde wet wordt door verschillende overheidsorganisaties anders geïnterpreteerd en toegepast, wat leidt tot inconsistenties en onrecht.',
          solutionTitle: 'Eenduidige toepassing',
          solutionText:
            'Zouden machine-uitvoerbare wetten interpretatieproblemen kunnen verminderen? We onderzoeken of dit kan leiden tot consistentere regeltoepassing.',
        },
        {
          problemTitle: 'Ondoorzichtige beslissingen',
          problemText:
            'Burgers krijgen besluiten zonder uitleg over hoe deze tot stand zijn gekomen. Overheid als black box.',
          solutionTitle: 'Volledige traceerbaarheid',
          solutionText:
            'Kunnen we elk besluit traceerbaar maken naar de exacte regeltoepassing? We verkennen mogelijkheden voor meer transparantie in overheidsbeslissingen.',
        },
        {
          problemTitle: 'Onuitvoerbare wetten',
          problemText:
            'Wetten worden vaak geschreven zonder volledig te testen of ze in de praktijk uitvoerbaar zijn. Dit kan leiden tot implementatieproblemen door inconsistenties, onduidelijkheden of praktische beperkingen.',
          solutionTitle: 'Uitvoerbaarheid testen',
          solutionText:
            'Zou machine-uitvoerbare wetgeving het mogelijk maken om wetten te testen? We onderzoeken of inconsistenties en conflicten vroegtijdig gedetecteerd kunnen worden.',
        },
      ],
    },
    howItWorks: {
      title: 'Van analoog recht naar digitaal rechtsstelsel',
      lede: 'Hoe zou de overgang van traditionele wetgeving naar een digitaal rechtsstelsel kunnen verlopen? We verkennen zeven mogelijke stappen en onderzoeken wat daardoor mogelijk zou kunnen worden:',
      steps: [
        {
          title: 'Analoog naar digitaal',
          text: 'Kunnen bestaande wetten systematisch worden omgezet van analoge tekst naar machine-uitvoerbare specificaties? Een eerste stap om een digitale basis te onderzoeken.',
        },
        {
          title: 'Digitaal rechtsstelsel',
          text: 'Zouden nieuwe wetten vanaf het begin machine-uitvoerbaar kunnen worden geschreven? We verkennen hoe dat eruit zou kunnen zien en hoe we daarin kunnen ondersteunen.',
        },
        {
          title: 'Gestandaardiseerd ecosysteem',
          text: 'Een landelijke infrastructuur waar alle overheidssystemen dezelfde wettelijke definities gebruiken. Eén bron van waarheid voor regeltoepassing.',
        },
        {
          title: 'Harmonisatie van wetgeving',
          text: 'Het wordt mogelijk om systematisch te werken aan harmonisatie van bestaande wetgeving. Conflicten en inconsistenties tussen bestaande regelsets kunnen automatisch worden gedetecteerd, waardoor harmonisatie een bewuste keuze wordt.',
        },
        {
          title: 'Uitvoerbaarheidstoets',
          text: 'Nieuwe wetten kunnen worden getest voordat ze ingaan. Het effect van nieuwe wetgeving op de consistentie van het rechtsstelsel kan tijdens het wetgevingsproces worden geanalyseerd.',
        },
        {
          title: 'Centrale publicatie',
          text: 'Machine-uitvoerbare wetgeving wordt centraal gepubliceerd voor iedereen. Execution engines worden ook beschikbaar gesteld zodat alle partijen dezelfde wetten op identieke wijze kunnen uitvoeren.',
        },
        {
          title: 'Transparante toepassing',
          text: 'Burgers en bedrijven kunnen de exacte werking van regels inzien en controleren. Volledige inzichtelijkheid in regeltoepassing.',
        },
      ],
    },
    tools: {
      title: 'Ecosysteem',
      items: [
        {
          title: 'NRML-taal',
          link: { label: 'GitHub', href: 'https://github.com/MinBZK/NRML' },
          text: 'Normalized Rule Model Language — een JSON-gebaseerd uitwisselings- en opslagformaat voor machine-uitvoerbare wetten. NRML maakt complexe juridische logica toegankelijk, ondersteunt meertalige regels en heeft ingebouwde versioning.',
        },
        {
          title: 'Execution engines',
          meta: 'Python • Go • meer komen',
          text: 'Meerdere execution engines die NRML-code uitvoeren. Verschillende talen voor verschillende use cases — maar consistent juridisch resultaat.',
        },
        {
          title: 'Analoog-recht-converter',
          meta: 'AI-ondersteund',
          text: 'LLM-gebaseerde tool om bestaand analoog recht om te zetten naar machine-uitvoerbare NRML. Automatische transformatie van traditionele wetgeving naar een digitaal rechtsstelsel.',
        },
        {
          title: 'Visual law editor',
          meta: 'Blockly-gebaseerd',
          text: 'Intuïtieve drag-and-drop-interface voor juristen om wetten te schrijven zonder programmeerkennis. Van visuele logica naar uitvoerbare code.',
        },
        {
          title: 'Dependency mapper',
          meta: 'Relatie-analyse',
          text: 'Visualiseert complexe relaties tussen verschillende wetten. Toont hoe wijzigingen in één wet doorwerken in het gehele juridische ecosysteem.',
        },
        {
          title: 'Regelbeheer',
          meta: 'Git-gebaseerd',
          text: 'Volledige versioning van machine-uitvoerbare wetgeving. Track wijzigingen, rollback-mogelijkheden, en branch verschillende wetsvoorstellen.',
        },
        {
          title: 'Simulatieomgeving',
          link: {
            label: 'Live demo',
            href: 'https://ui.lac.apps.digilab.network/simulation',
          },
          text: 'Test de gevolgen van nieuwe wetgeving vóór implementatie. Model maatschappelijke impact en voorspel onbedoelde effecten.',
        },
        {
          title: 'Publicatieplatform',
          meta: 'API + web',
          text: 'Centrale publicatie en distributie van machine-uitvoerbare wetgeving. API-toegang voor alle overheidssystemen en private partijen.',
        },
      ],
    },
    example: {
      title: 'Hoe zou dit eruit kunnen zien?',
      lede: 'Wat zouden de mogelijkheden van het RegelRecht-ecosysteem in de praktijk kunnen zijn? We verkennen twee denkrichtingen voor transparante regeltoepassing en wetgevingstesting.',
      cases: [
        {
          img: '/burger-nl-screenshot.png',
          alt: 'Schermafbeelding van een persoonlijk regeldashboard: een lijst met toeslagen en uitkeringen waarbij per regel de herkomst in de wet wordt getoond.',
          caption:
            'Concept: een persoonlijk dashboard waarin elke uitkomst herleidbaar is naar de onderliggende wet.',
          h: 'Persoonlijk regeldashboard',
          p: 'Wat als burgers op één plek al hun toeslagen, uitkeringen en verplichtingen zouden kunnen zien? Elke regel zou dan traceerbaar kunnen zijn terug naar de machine-uitvoerbare wetgeving, met volledige transparantie over hoe besluiten tot stand komen.',
          bullets: [
            'Real-time regeltoepassing — zou directe feedback mogelijk maken?',
            'Volledige traceerbaarheid — kan een pad van wet naar persoonlijke situatie gelegd worden?',
            'Proactieve communicatie — kunnen burgers automatisch geïnformeerd worden bij regelwijzigingen?',
          ],
        },
        {
          img: '/simulatie-screenshot.png',
          alt: 'Schermafbeelding van een simulatieomgeving waarin het effect van een wetswijziging op verschillende voorbeeldsituaties wordt doorgerekend.',
          caption: 'Concept: nieuwe wetgeving doorrekenen vóór invoering.',
          h: 'Wetgeving simulatie & testing',
          p: 'Wat als beleidsmakers de gevolgen van nieuwe wetgeving zouden kunnen testen in een simulatieomgeving voordat deze wordt ingevoerd? Zou dit onbedoelde effecten kunnen voorkomen en de kwaliteit van wetgeving kunnen verbeteren?',
          bullets: [
            'Impactanalyse — zouden we de gevolgen van nieuwe regelgeving kunnen voorspellen?',
            'Harmonisatiecontrole — kunnen we conflicten met bestaande wetgeving detecteren?',
            'Scenariotesting — is het mogelijk verschillende beleidsopties te testen?',
            'Kwaliteitscontrole — kunnen inconsistenties vóór implementatie worden gedetecteerd?',
          ],
          reverse: true,
        },
      ],
    },
    innovation: {
      title: 'Verkenning binnen Innovatiebudget 2025',
      ledeBefore: 'RegelRecht draagt bij aan twee projecten uit het ',
      ledeLink: {
        label: 'Innovatiebudget 2025',
        href: 'https://www.digitaleoverheid.nl/overzicht-van-alle-onderwerpen/innovatie/innovatiebudget/toekenning-innovatiebudget-2025/',
      },
      ledeAfter: ' van de Digitale Overheid:',
      cards: [
        {
          meta: 'In samenwerking met VNG',
          h: 'Minder burgers in de knel door machineleesbare wetgeving',
          p: 'Hoe kunnen we voorkomen dat de stapeling van wet- en regelgeving wetten onuitvoerbaar maakt? Dit project verkent de ontwikkeling van een analysetool om wetsvoorstellen te toetsen op uitvoerbaarheid in samenhang met andere wetten.',
        },
        {
          meta: 'In samenwerking met Dienst Toeslagen',
          h: 'Modern rekenhart als bouwsteen voor de hele overheid',
          p: "Kunnen we een algemeen rekenhart voor de overheid ontwikkelen? Dit project verkent hoe zo'n systeem zou kunnen helpen bij het uitvoeren van complexe regelingen voor burgers en bedrijven, bijvoorbeeld bij het berekenen van toeslagen.",
        },
      ],
    },
    references: {
      title: 'Relevante rapporten en bronnen',
      lede: 'Een overzicht van belangrijke rapporten, onderzoeken en bronnen die de noodzaak voor machine-uitvoerbare wetgeving onderbouwen.',
      items: [
        {
          title: 'Factsheet digitale uitvoering van wetgeving',
          meta: 'Prof. Corien Prins (WRR) & Prof. Johan Wolswinkel (Tilburg University) • 23 januari 2025',
          text: 'Deze WRR-factsheet identificeert vijf aandachtspunten en toetsvragen voor parlementaire controle op digitale uitvoering van wetgeving. Het RegelRecht-project valt binnen het domein van deze factsheet en kan worden beoordeeld aan de hand van de voorgestelde criteria voor transparantie, traceerbaarheid en democratische controle.',
          href: 'https://www.wrr.nl/actueel/nieuws/2025/01/23/factsheet-digitale-uitvoering-van-wetgeving',
          linkLabel: 'WRR',
        },
        {
          title:
            "Factsheet rechtsstatelijke risico's van de digitale uitvoering van wetten",
          meta: 'Dr. Mariette Lokin (OU/VU) & prof. mr. dr. Reijer Passchier (OU/Universiteit Leiden) • 29 november 2024',
          text: "Deze factsheet voor de Vaste commissie Digitale Zaken van de Tweede Kamer benoemt zes rechtsstatelijke risico's van digitale wetsuitvoering, waaronder ondoorzichtigheid en vertaalproblemen tussen wettekst en code, en bepleit traceerbaarheid van algoritmen naar hun juridische bron.",
          href: 'https://www.eerstekamer.nl/bijlage/20250129/wetenschappelijke_factsheet/document3/f=/vmkgn0uje7le.pdf',
          linkLabel: 'PDF',
        },
        {
          title: 'Informatiehuishouding, de postkoets met hulpmotor',
          meta: 'Arre Zuurmond (Regeringscommissaris) • 1 mei 2023',
          text: 'Zuurmond signaleert dat de huidige informatiehuishouding een bureaucratische, reactieve overheid ondersteunt die te sterk gebaseerd is op wantrouwen jegens burgers. Hij pleit voor een responsieve overheid met betere informatievoorziening.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2023/05/01/rapportage-regeringscommissaris-informatiehuishouding',
          linkLabel: 'Rapport',
        },
        {
          title: 'Algoritmes getoetst',
          meta: 'Algemene Rekenkamer • 18 mei 2022',
          text: "De Rekenkamer testte 9 algoritmes bij verschillende overheidsorganisaties en constateerde dat 6 daarvan risico's hadden op het gebied van prestatiebeheer, bias, datalekken of onbevoegde toegang. Het rapport benadrukt de noodzaak van continue monitoring.",
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2022/05/18/algoritmes-getoetst',
          linkLabel: 'Rapport',
        },
        {
          title: 'Aandacht voor algoritmes',
          meta: 'Algemene Rekenkamer • 26 januari 2021',
          text: 'Dit eerste systematische onderzoek naar algoritmegebruik bij de Nederlandse overheid constateerde dat algoritmes zich vooral richten op overheidsbehoeften, met beperkte aandacht voor ethische aspecten en burgerinzicht.',
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2021/01/26/aandacht-voor-algoritmes',
          linkLabel: 'Rapport',
        },
        {
          title: 'Aanbevelingen wetgevingsproces en wetgevingskwaliteit',
          meta: 'Raad van State (Afdeling advisering) • 19 april 2021',
          text: 'De Raad van State benadrukt het belang van uitvoeringstoetsen en samenwerking tussen beleidsmakers, wetgevingsjuristen en uitvoeringsorganisaties in multidisciplinaire teams, en pleit voor betere toetsing op uitvoerbaarheid en doenvermogen.',
          href: 'https://www.raadvanstate.nl/actueel/nieuws/@125178/aanbevelingen-wetgevingsproces',
          linkLabel: 'Aanbevelingen',
        },
        {
          title:
            'Gematigde groei — Staatscommissie Demografische Ontwikkelingen 2050',
          meta: 'Staatscommissie o.v.v. Richard van Zwol • 15 januari 2024',
          text: 'De staatscommissie signaleert dat demografische ontwikkelingen leiden tot druk op toegankelijkheid van overheidsdiensten zoals onderwijs, zorg en huisvesting.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2024/01/15/gematigde-groei-rapport-van-de-staatscommissie-demografische-ontwikkleingen-2050',
          linkLabel: 'Rapport',
        },
        {
          title: 'Maak waar! — De digitale overheid',
          meta: 'Studiegroep Informatiesamenleving en Overheid (o.v.v. Richard van Zwol) • 18 april 2017',
          text: 'De studiegroep concludeert dat digitalisering van de overheid een radicale mentaliteitsverandering vereist en dat digitale dienstverlening tot de kern van het primaire proces hoort.',
          href: 'https://kennisopenbaarbestuur.nl/documenten/rapporten/2017/04/18/maak-waar',
          linkLabel: 'Rapport',
        },
        {
          title: 'Werk aan Uitvoering — Fase 2: Handelingsperspectieven',
          meta: 'Interdepartementaal (BZK, Financiën, OCW, SZW) • 3 juli 2020',
          text: 'Dit rapport analyseert problemen bij uitvoeringsorganisaties zoals de Belastingdienst, DUO en UWV: continuïteitsrisico’s, beperkte wendbaarheid bij beleidswijzigingen, en ontbrekende mogelijkheden voor maatwerk.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2020/07/03/werk-aan-uitvoering-fase-2-handelingsperspectieven-en-samenvatting-analyse',
          linkLabel: 'Rapport',
        },
        {
          title:
            'Open op orde: generiek actieplan informatiehuishouding Rijksoverheid',
          meta: 'Ministerie van BZK • 6 april 2021',
          text: "Dit actieplan werd opgesteld als reactie op het rapport 'Ongekend onrecht' en richt zich op structurele verbetering van de informatiehuishouding binnen de gehele rijksoverheid.",
          href: 'https://www.eerstekamer.nl/overig/20210406/open_op_orde_generiek_actieplan/document3/f=/vlhqp2mq5pvc.pdf',
          linkLabel: 'PDF',
        },
      ],
    },
    faq: {
      title: 'Vragen bij deze verkenning',
      items: [
        {
          q: 'Wat zou een digitaal rechtsstelsel kunnen betekenen?',
          a: 'Een digitaal rechtsstelsel zou kunnen betekenen dat juridische regels worden geschreven als uitvoerbare code die computers direct zouden kunnen draaien en toepassen, zonder tussenkomst van menselijke interpretatie of programmeurs. Maar is dit realiseerbaar? En hoe verhoudt dit zich tot traditioneel analoog recht?',
        },
        {
          q: 'Waarom NRML en niet andere regeltalen?',
          a: 'NRML onderscheidt zich door een ontwikkelaarsvriendelijke JSON-benadering in plaats van complexe XML-standaarden. Het dient als standaard uitwisselings- en opslagformaat zodat verschillende overheidssystemen dezelfde regelspecificaties kunnen gebruiken. NRML heeft ingebouwde testscenario’s en versioning.',
        },
        {
          q: 'Hoe zou dit zich kunnen verhouden tot bestaande systemen?',
          a: 'Zou RegelRecht bestaande implementaties kunnen valideren en kunnen dienen als referentie voor nieuwe systemen? Het zou bestaande systemen niet direct vervangen, maar zou controle en modernisering mogelijk kunnen maken.',
        },
        {
          q: 'Zou RegelRecht juridisch bindend kunnen zijn?',
          a: 'RegelRecht zou een technisch hulpmiddel kunnen zijn. De juridische geldigheid zou bij de oorspronkelijke wetgeving blijven. Maar zou het kunnen helpen bij consistente interpretatie en toepassing?',
        },
        {
          q: 'Hoe draagt dit bij aan transparantie?',
          a: 'Door regels expliciet te maken in code kunnen burgers en organisaties exact zien hoe beslissingen tot stand komen, in plaats van te vertrouwen op ondoorzichtige systemen.',
        },
      ],
    },
    feedback: {
      title: 'Wat denk jij?',
      body: 'Deze verkenning van machine-uitvoerbare wetgeving roept veel vragen op. Hoe zie jij de toekomst van de digitale overheid? Wat zijn je zorgen en verwachtingen bij deze ontwikkelingen? Jouw input helpt ons deze verkenning verder vorm te geven.',
      cta: 'Meld je aan of deel je gedachten',
      ctaHref: SIGNUP_NL_PATH,
    },
    footer: {
      blurb:
        'Een verkenning van Bureau Architectuur van het Ministerie van Binnenlandse Zaken naar de mogelijkheden van transparante, uitvoerbare wetgeving.',
      linksTitle: 'Links',
      contactTitle: 'Contact',
      partOfTitle: 'Onderdeel van',
      copyright:
        '© 2026 Ministerie van Binnenlandse Zaken en Koninkrijksrelaties. Alle rechten voorbehouden.',
      links: [
        { label: 'GitHub-repository', href: GITHUB },
        { label: 'Hoe het werkt', href: '/#how-it-works' },
        { label: 'Op de hoogte blijven', href: SIGNUP_NL_PATH },
        { label: 'Documentatie (Engels)', href: GUIDE_PATH },
        { label: 'Toegankelijkheid', href: '/reference/toegankelijkheid' },
      ],
      partOf: [
        'Bureau Architectuur',
        'Ministerie van Binnenlandse Zaken en Koninkrijksrelaties',
      ],
    },
    signup: {
      pageTitle: 'Op de hoogte blijven of bijdragen aan RegelRecht?',
      lede: 'Laat je gegevens achter als je updates wilt ontvangen of wilt meedenken over de (juridische) validatie van RegelRecht.',
      noscript:
        'Dit formulier heeft JavaScript nodig. Stuur in plaats daarvan een e-mail naar regelrecht@minbzk.nl.',
      legend:
        'Wil je bijdragen aan de (juridische) validatie van RegelRecht?',
      radioYes: 'Ja, ik wil bijdragen aan de validatie van RegelRecht',
      radioNo: 'Nee, ik wil niet bijdragen',
      updates:
        'Ik wil updates ontvangen over de ontwikkelingen van RegelRecht',
      emailLabel: 'E-mailadres',
      nameLabel: 'Volledige naam',
      orgLabel: 'Organisatie (optioneel)',
      orgPlaceholder: 'Bijv. Ministerie van BZK, Gemeente Amsterdam',
      roleLabel: 'Functie (optioneel)',
      rolePlaceholder: 'Bijv. jurist, beleidsmedewerker, wetgevingsjurist',
      required: '*',
      requiredSr: '(verplicht)',
      submit: 'Meld me aan',
      submitting: 'Bezig met versturen…',
      companyHoneypot: 'Bedrijf (niet invullen)',
      errEmailEmpty: 'Vul je e-mailadres in.',
      errEmailInvalid: 'Vul een geldig e-mailadres in.',
      errName: 'Vul je volledige naam in.',
      successTitle: 'Bedankt voor je aanmelding!',
      successBody:
        'We hebben je gegevens verstuurd. Je ontvangt bevestiging per e-mail zodra je aanmelding is verwerkt. Klopt er iets niet? Mail dan naar regelrecht@minbzk.nl.',
      successReset: 'Nog iemand aanmelden',
      errorTitle: 'Er ging iets mis',
      errorBody:
        'Het versturen is niet gelukt. Probeer het opnieuw of stuur een e-mail naar regelrecht@minbzk.nl.',
      errorReset: 'Opnieuw proberen',
    },
  },

  en: {
    meta: {
      title: 'RegelRecht — from statute to digital execution',
      description:
        'An exploration by the Dutch Ministry of the Interior into transparent, machine-executable legislation.',
    },
    nav: {
      brandTagline: 'An exploration by the Dutch Ministry of the Interior',
      what: 'What',
      how: 'How',
      tools: 'Ecosystem',
      example: 'Examples',
      faq: 'Questions',
      signup: 'Sign up',
      docs: 'Documentation',
    },
    hero: {
      titleSmall: 'from statute to digital execution',
      intro:
        'RegelRecht explores how we can deliver transparent, unambiguous and consistent execution of legislation. An exploration of digital possibilities where everyone can understand how decisions are reached.',
      cta: 'Explore the possibilities',
    },
    partners: {
      label: 'An initiative of',
      items: [
        {
          label: 'Ministry of the Interior and Kingdom Relations',
          href: 'https://www.rijksoverheid.nl/ministeries/ministerie-van-binnenlandse-zaken-en-koninkrijksrelaties',
        },
        { label: 'Bureau Architectuur', href: 'https://minbzk.github.io/BASE/' },
        { label: 'Digilab', href: 'https://digilab.overheid.nl/' },
      ],
    },
    whatIsIt: {
      title: 'What is RegelRecht?',
      lede: 'Executing legislation comes with several challenges: differing interpretations, opaque systems, and complex programming work that often sits far from the original law. RegelRecht explores whether machine-executable legislation can offer an answer — laws written directly as executable code, without programmers in between.',
      cards: [
        {
          h: 'From analogue law to code',
          p: 'Can we transform traditional legislation into machine-executable specifications? We are investigating whether this can narrow the gap between legislator and execution.',
        },
        {
          h: 'A single source of truth',
          p: 'What if there were one central, machine-executable version of every law that all parties use? We are exploring whether this can reduce differences in interpretation.',
        },
        {
          h: 'Full transparency',
          p: 'How do we make government decisions more transparent? We are experimenting with ways for citizens to see which rules apply and how decisions are reached.',
        },
      ],
    },
    whyImportant: {
      title: 'Why this exploration?',
      lede: 'The current way laws are applied raises several challenges for the rule of law. We are investigating whether new technical approaches can contribute to solutions for these structural questions.',
      problemSolutions: [
        {
          problemTitle: 'Differing interpretations',
          problemText:
            'The same law is interpreted and applied differently by different government organisations, leading to inconsistencies and injustice.',
          solutionTitle: 'Unambiguous application',
          solutionText:
            'Could machine-executable laws reduce interpretation problems? We are investigating whether this can lead to more consistent rule application.',
        },
        {
          problemTitle: 'Opaque decisions',
          problemText:
            'Citizens receive decisions with no explanation of how they were reached. Government as a black box.',
          solutionTitle: 'Full traceability',
          solutionText:
            'Can we make every decision traceable back to the exact rule that was applied? We are exploring options for more transparency in government decisions.',
        },
        {
          problemTitle: 'Unworkable laws',
          problemText:
            'Laws are often written without fully testing whether they are workable in practice. This can cause implementation problems through inconsistencies, ambiguities or practical constraints.',
          solutionTitle: 'Testing workability',
          solutionText:
            'Would machine-executable legislation make it possible to test laws? We are investigating whether inconsistencies and conflicts can be detected early.',
        },
      ],
    },
    howItWorks: {
      title: 'From analogue law to a digital legal system',
      lede: 'How might the transition from traditional legislation to a digital legal system unfold? We explore seven possible steps and what each could make possible:',
      steps: [
        {
          title: 'Analogue to digital',
          text: 'Can existing laws be systematically converted from analogue text into machine-executable specifications? A first step to explore a digital foundation.',
        },
        {
          title: 'Digital legal system',
          text: 'Could new laws be written machine-executable from the start? We explore what that could look like and how we can support it.',
        },
        {
          title: 'Standardised ecosystem',
          text: 'A national infrastructure where all government systems use the same legal definitions. A single source of truth for rule application.',
        },
        {
          title: 'Harmonising legislation',
          text: 'It becomes possible to work systematically on harmonising existing legislation. Conflicts and inconsistencies between existing rule sets can be detected automatically, making harmonisation a deliberate choice.',
        },
        {
          title: 'Workability assessment',
          text: 'New laws can be tested before they take effect. The effect of new legislation on the consistency of the legal system can be analysed during the legislative process.',
        },
        {
          title: 'Central publication',
          text: 'Machine-executable legislation is published centrally for everyone. Execution engines are made available too, so all parties can run the same laws in an identical way.',
        },
        {
          title: 'Transparent application',
          text: 'Citizens and businesses can inspect and verify exactly how rules work. Full transparency into rule application.',
        },
      ],
    },
    tools: {
      title: 'Ecosystem',
      items: [
        {
          title: 'NRML language',
          link: { label: 'GitHub', href: 'https://github.com/MinBZK/NRML' },
          text: 'Normalized Rule Model Language — a JSON-based exchange and storage format for machine-executable laws. NRML makes complex legal logic accessible, supports multilingual rules and has built-in versioning.',
        },
        {
          title: 'Execution engines',
          meta: 'Python • Go • more to come',
          text: 'Multiple execution engines that run NRML code. Different languages for different use cases — but a consistent legal result.',
        },
        {
          title: 'Analogue-law converter',
          meta: 'AI-assisted',
          text: 'An LLM-based tool to convert existing analogue law into machine-executable NRML. Automatic transformation of traditional legislation into a digital legal system.',
        },
        {
          title: 'Visual law editor',
          meta: 'Blockly-based',
          text: 'An intuitive drag-and-drop interface for legal experts to write laws without programming knowledge. From visual logic to executable code.',
        },
        {
          title: 'Dependency mapper',
          meta: 'Relationship analysis',
          text: 'Visualises complex relationships between different laws. Shows how changes to one law propagate through the entire legal ecosystem.',
        },
        {
          title: 'Rule management',
          meta: 'Git-based',
          text: 'Full versioning of machine-executable legislation. Track changes, roll back, and branch different legislative proposals.',
        },
        {
          title: 'Simulation environment',
          link: {
            label: 'Live demo',
            href: 'https://ui.lac.apps.digilab.network/simulation',
          },
          text: 'Test the consequences of new legislation before implementation. Model societal impact and predict unintended effects.',
        },
        {
          title: 'Publication platform',
          meta: 'API + web',
          text: 'Central publication and distribution of machine-executable legislation. API access for all government systems and private parties.',
        },
      ],
    },
    example: {
      title: 'What could this look like?',
      lede: 'What could the RegelRecht ecosystem make possible in practice? We explore two directions for transparent rule application and legislative testing.',
      cases: [
        {
          img: '/burger-nl-screenshot.png',
          alt: 'Screenshot of a personal rules dashboard: a list of benefits and allowances where each rule shows its origin in the law.',
          caption:
            'Concept: a personal dashboard where every outcome is traceable back to the underlying law.',
          h: 'Personal rules dashboard',
          p: 'What if citizens could see all their benefits, allowances and obligations in one place? Every rule could then be traceable back to the machine-executable legislation, with full transparency about how decisions are reached.',
          bullets: [
            'Real-time rule application — could it make immediate feedback possible?',
            'Full traceability — can a path be drawn from law to personal situation?',
            'Proactive communication — can citizens be informed automatically when rules change?',
          ],
        },
        {
          img: '/simulatie-screenshot.png',
          alt: 'Screenshot of a simulation environment that computes the effect of a legislative change across different example situations.',
          caption: 'Concept: running new legislation through the numbers before it takes effect.',
          h: 'Legislative simulation & testing',
          p: 'What if policy makers could test the consequences of new legislation in a simulation environment before it is introduced? Could this prevent unintended effects and improve the quality of legislation?',
          bullets: [
            'Impact analysis — could we predict the consequences of new regulation?',
            'Harmonisation check — can we detect conflicts with existing legislation?',
            'Scenario testing — is it possible to test different policy options?',
            'Quality control — can inconsistencies be detected before implementation?',
          ],
          reverse: true,
        },
      ],
    },
    innovation: {
      title: 'Exploration within the 2025 Innovation Budget',
      ledeBefore: 'RegelRecht contributes to two projects from the ',
      ledeLink: {
        label: '2025 Innovation Budget',
        href: 'https://www.digitaleoverheid.nl/overzicht-van-alle-onderwerpen/innovatie/innovatiebudget/toekenning-innovatiebudget-2025/',
      },
      ledeAfter: ' of the Dutch Digital Government:',
      cards: [
        {
          meta: 'In collaboration with VNG',
          h: 'Fewer citizens in trouble through machine-readable legislation',
          p: 'How do we prevent the accumulation of laws and regulations from making laws unworkable? This project explores developing an analysis tool to test legislative proposals for workability in conjunction with other laws.',
        },
        {
          meta: 'In collaboration with Dienst Toeslagen',
          h: 'A modern calculation core as a building block for government',
          p: 'Can we develop a general calculation core for government? This project explores how such a system could help execute complex schemes for citizens and businesses, for example when calculating allowances.',
        },
      ],
    },
    references: {
      title: 'Relevant reports and sources',
      lede: 'An overview of key reports, research and sources that underpin the need for machine-executable legislation.',
      items: [
        {
          title: 'Factsheet on the digital execution of legislation',
          meta: 'Prof. Corien Prins (WRR) & Prof. Johan Wolswinkel (Tilburg University) • 23 January 2025',
          text: 'This WRR factsheet identifies five points of attention and review questions for parliamentary oversight of the digital execution of legislation. The RegelRecht project falls within the scope of this factsheet and can be assessed against the proposed criteria for transparency, traceability and democratic control.',
          href: 'https://www.wrr.nl/actueel/nieuws/2025/01/23/factsheet-digitale-uitvoering-van-wetgeving',
          linkLabel: 'WRR',
        },
        {
          title:
            'Factsheet on rule-of-law risks of the digital execution of laws',
          meta: 'Dr. Mariette Lokin (OU/VU) & Prof. Reijer Passchier (OU/Leiden University) • 29 November 2024',
          text: 'This factsheet for the House of Representatives’ Standing Committee on Digital Affairs names six rule-of-law risks of digital law execution, including opacity and translation problems between legal text and code, and argues for traceability of algorithms back to their legal source.',
          href: 'https://www.eerstekamer.nl/bijlage/20250129/wetenschappelijke_factsheet/document3/f=/vmkgn0uje7le.pdf',
          linkLabel: 'PDF',
        },
        {
          title: 'Information management, the stagecoach with an auxiliary motor',
          meta: 'Arre Zuurmond (Government Commissioner) • 1 May 2023',
          text: 'Zuurmond observes that current information management supports a bureaucratic, reactive government too strongly based on distrust of citizens. He argues for a responsive government with better information provision.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2023/05/01/rapportage-regeringscommissaris-informatiehuishouding',
          linkLabel: 'Report',
        },
        {
          title: 'Algorithms tested',
          meta: 'Netherlands Court of Audit • 18 May 2022',
          text: 'The Court of Audit tested 9 algorithms at various government organisations and found that 6 of them carried risks around performance management, bias, data leaks or unauthorised access. The report stresses the need for continuous monitoring.',
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2022/05/18/algoritmes-getoetst',
          linkLabel: 'Report',
        },
        {
          title: 'Attention to algorithms',
          meta: 'Netherlands Court of Audit • 26 January 2021',
          text: 'This first systematic study of algorithm use by the Dutch government found that algorithms focus mainly on government needs, with limited attention to ethical aspects and citizen insight.',
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2021/01/26/aandacht-voor-algoritmes',
          linkLabel: 'Report',
        },
        {
          title: 'Recommendations on the legislative process and quality',
          meta: 'Council of State (Advisory Division) • 19 April 2021',
          text: 'The Council of State stresses the importance of implementation assessments and collaboration between policy makers, legislative lawyers and implementing organisations in multidisciplinary teams, and argues for better testing of workability and citizens’ ability to act.',
          href: 'https://www.raadvanstate.nl/actueel/nieuws/@125178/aanbevelingen-wetgevingsproces',
          linkLabel: 'Recommendations',
        },
        {
          title:
            'Moderate growth — State Commission on Demographic Developments 2050',
          meta: 'State Commission chaired by Richard van Zwol • 15 January 2024',
          text: 'The State Commission observes that demographic developments put pressure on the accessibility of government services such as education, healthcare and housing.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2024/01/15/gematigde-groei-rapport-van-de-staatscommissie-demografische-ontwikkleingen-2050',
          linkLabel: 'Report',
        },
        {
          title: 'Make it happen! — The digital government',
          meta: 'Study Group on the Information Society and Government (chaired by Richard van Zwol) • 18 April 2017',
          text: 'The study group concludes that digitising government requires a radical change of mindset and that digital service delivery belongs at the core of the primary process.',
          href: 'https://kennisopenbaarbestuur.nl/documenten/rapporten/2017/04/18/maak-waar',
          linkLabel: 'Report',
        },
        {
          title: 'Work on Implementation — Phase 2: Courses of action',
          meta: 'Interdepartmental (BZK, Finance, OCW, SZW) • 3 July 2020',
          text: 'This report analyses problems at implementing organisations such as the Tax Administration, DUO and UWV: continuity risks, limited agility when policy changes, and missing options for tailored solutions.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2020/07/03/werk-aan-uitvoering-fase-2-handelingsperspectieven-en-samenvatting-analyse',
          linkLabel: 'Report',
        },
        {
          title:
            'Open and in order: generic action plan for information management',
          meta: 'Ministry of the Interior • 6 April 2021',
          text: "This action plan was drawn up in response to the 'Unprecedented injustice' report and focuses on structurally improving information management across central government.",
          href: 'https://www.eerstekamer.nl/overig/20210406/open_op_orde_generiek_actieplan/document3/f=/vlhqp2mq5pvc.pdf',
          linkLabel: 'PDF',
        },
      ],
    },
    faq: {
      title: 'Questions about this exploration',
      items: [
        {
          q: 'What could a digital legal system mean?',
          a: 'A digital legal system could mean that legal rules are written as executable code that computers can run and apply directly, without human interpretation or programmers in between. But is this achievable? And how does it relate to traditional analogue law?',
        },
        {
          q: 'Why NRML and not other rule languages?',
          a: 'NRML stands out through a developer-friendly JSON approach instead of complex XML standards. It serves as a standard exchange and storage format so that different government systems can use the same rule specifications. NRML has built-in test scenarios and versioning.',
        },
        {
          q: 'How could this relate to existing systems?',
          a: 'Could RegelRecht validate existing implementations and serve as a reference for new systems? It would not replace existing systems directly, but it could make verification and modernisation possible.',
        },
        {
          q: 'Could RegelRecht be legally binding?',
          a: 'RegelRecht could be a technical aid. Legal validity would remain with the original legislation. But could it help with consistent interpretation and application?',
        },
        {
          q: 'How does this contribute to transparency?',
          a: 'By making rules explicit in code, citizens and organisations can see exactly how decisions are reached, instead of relying on opaque systems.',
        },
      ],
    },
    feedback: {
      title: 'What do you think?',
      body: 'This exploration of machine-executable legislation raises many questions. How do you see the future of digital government? What are your concerns and expectations around these developments? Your input helps us shape this exploration further.',
      cta: 'Sign up or share your thoughts',
      ctaHref: SIGNUP_EN_PATH,
    },
    footer: {
      blurb:
        'An exploration by Bureau Architectuur of the Dutch Ministry of the Interior into the possibilities of transparent, executable legislation.',
      linksTitle: 'Links',
      contactTitle: 'Contact',
      partOfTitle: 'Part of',
      copyright:
        '© 2026 Ministry of the Interior and Kingdom Relations. All rights reserved.',
      links: [
        { label: 'GitHub repository', href: GITHUB },
        { label: 'How it works', href: '/en/#how-it-works' },
        { label: 'Stay informed', href: SIGNUP_EN_PATH },
        { label: 'Documentation', href: GUIDE_PATH },
        { label: 'Accessibility (Dutch)', href: '/reference/toegankelijkheid' },
      ],
      partOf: [
        'Bureau Architectuur',
        'Ministry of the Interior and Kingdom Relations',
      ],
    },
    signup: {
      pageTitle: 'Stay informed or contribute to RegelRecht?',
      lede: 'Leave your details if you want to receive updates or help think about the (legal) validation of RegelRecht.',
      noscript:
        'This form needs JavaScript. Please send an email to regelrecht@minbzk.nl instead.',
      legend: 'Would you like to contribute to the (legal) validation of RegelRecht?',
      radioYes: 'Yes, I want to contribute to the validation of RegelRecht',
      radioNo: 'No, I do not want to contribute',
      updates: 'I want to receive updates about the development of RegelRecht',
      emailLabel: 'Email address',
      nameLabel: 'Full name',
      orgLabel: 'Organisation (optional)',
      orgPlaceholder: 'E.g. Ministry of the Interior, City of Amsterdam',
      roleLabel: 'Role (optional)',
      rolePlaceholder: 'E.g. lawyer, policy officer, legislative lawyer',
      required: '*',
      requiredSr: '(required)',
      submit: 'Sign me up',
      submitting: 'Sending…',
      companyHoneypot: 'Company (do not fill in)',
      errEmailEmpty: 'Enter your email address.',
      errEmailInvalid: 'Enter a valid email address.',
      errName: 'Enter your full name.',
      successTitle: 'Thank you for signing up!',
      successBody:
        'We have sent your details. You will receive confirmation by email once your registration is processed. Something not right? Email regelrecht@minbzk.nl.',
      successReset: 'Sign up someone else',
      errorTitle: 'Something went wrong',
      errorBody:
        'Sending failed. Please try again or send an email to regelrecht@minbzk.nl.',
      errorReset: 'Try again',
    },
  },
}

export function langFromPath(path: string): 'nl' | 'en' {
  return path.startsWith('/en/') || path === '/en' ? 'en' : 'nl'
}
