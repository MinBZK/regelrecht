<script setup lang="ts">
import { useData } from 'vitepress'
import RrFooter from './RrFooter.vue'

const { frontmatter } = useData()
const showSignup = frontmatter.value.page === 'aanmelden'
</script>

<template>
  <div class="rr-landing" lang="nl">
    <a class="rr-skip-link" href="#rr-main">Direct naar de inhoud</a>

    <header class="rr-header">
      <nav class="rr-nav" aria-label="Hoofdnavigatie">
        <a class="rr-brand" href="/nl/">
          <b>RegelRecht</b>
          <small>Een verkenning van het Ministerie van BZK</small>
        </a>
        <ul class="rr-nav-links">
          <li v-if="!showSignup"><a href="#what-is-it">Wat</a></li>
          <li v-if="!showSignup"><a href="#how-it-works">Hoe</a></li>
          <li v-if="!showSignup"><a href="#tools">Ecosysteem</a></li>
          <li v-if="!showSignup"><a href="#example">Voorbeelden</a></li>
          <li v-if="!showSignup"><a href="#faq">Vragen</a></li>
          <li><a href="/nl/aanmelden">Aanmelden</a></li>
          <li><a href="/guide/what-is-regelrecht">Docs (EN)</a></li>
          <li>
            <a href="https://github.com/MinBZK/regelrecht">GitHub</a>
          </li>
        </ul>
      </nav>
    </header>

    <main id="rr-main">
      <slot v-if="$slots.default" />

      <!-- ===================== SIGNUP PAGE ===================== -->
      <template v-if="showSignup">
        <section class="rr-section" aria-labelledby="rr-signup-title">
          <div class="rr-container">
            <h1 id="rr-signup-title" class="rr-section-title">
              Op de hoogte blijven of bijdragen aan RegelRecht?
            </h1>
            <p class="rr-lede">
              Laat je gegevens achter als je updates wilt ontvangen of wilt
              meedenken over de (juridische) validatie van RegelRecht.
            </p>
            <ClientOnly>
              <SignupForm />
            </ClientOnly>
            <noscript>
              <p>
                Dit formulier heeft JavaScript nodig. Stuur in plaats daarvan
                een e-mail naar
                <a href="mailto:regelrecht@minbzk.nl">regelrecht@minbzk.nl</a>.
              </p>
            </noscript>
          </div>
        </section>
      </template>

      <!-- ===================== HOME / LANDING ===================== -->
      <template v-else>
        <section class="rr-hero" aria-labelledby="rr-hero-title">
          <div class="rr-container">
            <h1 id="rr-hero-title">
              RegelRecht
              <small>van wet naar digitale werking</small>
            </h1>
            <p>
              RegelRecht verkent hoe we transparante, eenduidige en consistente
              uitvoering van wetgeving kunnen realiseren. Een verkenning naar
              digitale mogelijkheden waarbij iedereen kan begrijpen hoe besluiten
              tot stand komen.
            </p>
            <p>
              <a class="rr-btn rr-btn--primary" href="#what-is-it">
                Verken de mogelijkheden
              </a>
            </p>
          </div>
        </section>

        <section class="rr-section rr-partners" aria-labelledby="rr-partners-title">
          <div class="rr-container">
            <h2 id="rr-partners-title" class="rr-visually-hidden">
              Een initiatief van
            </h2>
            <p>Een initiatief van</p>
            <ul>
              <li>
                <a href="https://www.rijksoverheid.nl/ministeries/ministerie-van-binnenlandse-zaken-en-koninkrijksrelaties">
                  Ministerie van Binnenlandse Zaken en Koninkrijksrelaties
                </a>
              </li>
              <li><a href="https://minbzk.github.io/BASE/">Bureau Architectuur</a></li>
              <li><a href="https://digilab.overheid.nl/">Digilab</a></li>
            </ul>
          </div>
        </section>

        <section
          id="what-is-it"
          class="rr-section rr-section--alt"
          aria-labelledby="rr-what-title"
        >
          <div class="rr-container">
            <h2 id="rr-what-title" class="rr-section-title">Wat is RegelRecht?</h2>
            <p class="rr-lede">
              De uitvoering van wetgeving kent verschillende uitdagingen:
              verschillende interpretaties, ondoorzichtige systemen en complex
              programmeerwerk dat vaak ver af staat van de oorspronkelijke wet.
              RegelRecht verkent of machine-uitvoerbare wetgeving een antwoord
              kan bieden — wetten die direct als uitvoerbare code geschreven
              worden, zonder tussenkomst van programmeurs.
            </p>
            <div class="rr-grid">
              <article class="rr-card">
                <h3>Van analoog recht naar code</h3>
                <p>
                  Kunnen we traditionele wetgeving transformeren naar
                  machine-uitvoerbare specificaties? We onderzoeken of dit de
                  kloof tussen wetgever en uitvoering kan verkleinen.
                </p>
              </article>
              <article class="rr-card">
                <h3>Eén bron van waarheid</h3>
                <p>
                  Wat als er één centrale, machine-uitvoerbare versie van elke
                  wet bestaat die alle partijen gebruiken? We verkennen of dit
                  interpretatieverschillen kan verminderen.
                </p>
              </article>
              <article class="rr-card">
                <h3>Volledige transparantie</h3>
                <p>
                  Hoe maken we overheidsbesluiten inzichtelijker? We
                  experimenteren met manieren waarop burgers kunnen zien welke
                  regels gelden en hoe beslissingen tot stand komen.
                </p>
              </article>
            </div>
          </div>
        </section>

        <section
          id="why-important"
          class="rr-section"
          aria-labelledby="rr-why-title"
        >
          <div class="rr-container">
            <h2 id="rr-why-title" class="rr-section-title">
              Waarom deze verkenning?
            </h2>
            <p class="rr-lede">
              De huidige manier waarop wetten worden toegepast kent verschillende
              uitdagingen in onze rechtsstaat. We onderzoeken of nieuwe technische
              benaderingen kunnen bijdragen aan oplossingen voor deze structurele
              vraagstukken.
            </p>
            <div class="rr-ps-list">
              <div
                v-for="ps in problemSolutions"
                :key="ps.problemTitle"
                class="rr-ps"
                role="group"
                :aria-label="ps.problemTitle + ' — ' + ps.solutionTitle"
              >
                <div class="rr-ps-problem">
                  <span class="rr-ps-tag">Het probleem</span>
                  <h3>{{ ps.problemTitle }}</h3>
                  <p>{{ ps.problemText }}</p>
                </div>
                <div class="rr-ps-solution">
                  <span class="rr-ps-tag">Mogelijke richting</span>
                  <h3>{{ ps.solutionTitle }}</h3>
                  <p>{{ ps.solutionText }}</p>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section
          id="how-it-works"
          class="rr-section rr-section--alt"
          aria-labelledby="rr-how-title"
        >
          <div class="rr-container">
            <h2 id="rr-how-title" class="rr-section-title">
              Van analoog recht naar digitaal rechtsstelsel
            </h2>
            <p class="rr-lede">
              Hoe zou de overgang van traditionele wetgeving naar een digitaal
              rechtsstelsel kunnen verlopen? We verkennen zeven mogelijke stappen
              en onderzoeken wat daardoor mogelijk zou kunnen worden:
            </p>
            <ol class="rr-steps">
              <li v-for="step in steps" :key="step.title">
                <h3>{{ step.title }}</h3>
                <p>{{ step.text }}</p>
              </li>
            </ol>
          </div>
        </section>

        <section id="tools" class="rr-section" aria-labelledby="rr-tools-title">
          <div class="rr-container">
            <h2 id="rr-tools-title" class="rr-section-title">Ecosysteem</h2>
            <div class="rr-grid">
              <article v-for="tool in tools" :key="tool.title" class="rr-card">
                <a
                  v-if="tool.link"
                  class="rr-card-link"
                  :href="tool.link.href"
                  >{{ tool.link.label }}</a
                >
                <span v-else class="rr-card-meta">{{ tool.meta }}</span>
                <h3>{{ tool.title }}</h3>
                <p>{{ tool.text }}</p>
              </article>
            </div>
          </div>
        </section>

        <section
          id="example"
          class="rr-section rr-section--alt"
          aria-labelledby="rr-example-title"
        >
          <div class="rr-container">
            <h2 id="rr-example-title" class="rr-section-title">
              Hoe zou dit eruit kunnen zien?
            </h2>
            <p class="rr-lede">
              Wat zouden de mogelijkheden van het RegelRecht-ecosysteem in de
              praktijk kunnen zijn? We verkennen twee denkrichtingen voor
              transparante regeltoepassing en wetgevingstesting.
            </p>

            <div class="rr-example">
              <figure>
                <img
                  src="/burger-nl-screenshot.png"
                  alt="Schermafbeelding van een persoonlijk regeldashboard: een lijst met toeslagen en uitkeringen waarbij per regel de herkomst in de wet wordt getoond."
                />
                <figcaption>
                  Concept: een persoonlijk dashboard waarin elke uitkomst
                  herleidbaar is naar de onderliggende wet.
                </figcaption>
              </figure>
              <div>
                <h3>Persoonlijk regeldashboard</h3>
                <p>
                  Wat als burgers op één plek al hun toeslagen, uitkeringen en
                  verplichtingen zouden kunnen zien? Elke regel zou dan
                  traceerbaar kunnen zijn terug naar de machine-uitvoerbare
                  wetgeving, met volledige transparantie over hoe besluiten tot
                  stand komen.
                </p>
                <ul>
                  <li>
                    <strong>Real-time regeltoepassing</strong> — zou directe
                    feedback mogelijk maken?
                  </li>
                  <li>
                    <strong>Volledige traceerbaarheid</strong> — kan een pad van
                    wet naar persoonlijke situatie gelegd worden?
                  </li>
                  <li>
                    <strong>Proactieve communicatie</strong> — kunnen burgers
                    automatisch geïnformeerd worden bij regelwijzigingen?
                  </li>
                </ul>
              </div>
            </div>

            <div class="rr-example rr-example--reverse">
              <figure>
                <img
                  src="/simulatie-screenshot.png"
                  alt="Schermafbeelding van een simulatieomgeving waarin het effect van een wetswijziging op verschillende voorbeeldsituaties wordt doorgerekend."
                />
                <figcaption>
                  Concept: nieuwe wetgeving doorrekenen vóór invoering.
                </figcaption>
              </figure>
              <div>
                <h3>Wetgeving simulatie &amp; testing</h3>
                <p>
                  Wat als beleidsmakers de gevolgen van nieuwe wetgeving zouden
                  kunnen testen in een simulatieomgeving voordat deze wordt
                  ingevoerd? Zou dit onbedoelde effecten kunnen voorkomen en de
                  kwaliteit van wetgeving kunnen verbeteren?
                </p>
                <ul>
                  <li>
                    <strong>Impactanalyse</strong> — zouden we de gevolgen van
                    nieuwe regelgeving kunnen voorspellen?
                  </li>
                  <li>
                    <strong>Harmonisatiecontrole</strong> — kunnen we conflicten
                    met bestaande wetgeving detecteren?
                  </li>
                  <li>
                    <strong>Scenariotesting</strong> — is het mogelijk
                    verschillende beleidsopties te testen?
                  </li>
                  <li>
                    <strong>Kwaliteitscontrole</strong> — kunnen inconsistenties
                    vóór implementatie worden gedetecteerd?
                  </li>
                </ul>
              </div>
            </div>
          </div>
        </section>

        <section
          id="innovation"
          class="rr-section"
          aria-labelledby="rr-innovation-title"
        >
          <div class="rr-container">
            <h2 id="rr-innovation-title" class="rr-section-title">
              Verkenning binnen Innovatiebudget 2025
            </h2>
            <p class="rr-lede">
              RegelRecht draagt bij aan twee projecten uit het
              <a
                href="https://www.digitaleoverheid.nl/overzicht-van-alle-onderwerpen/innovatie/innovatiebudget/toekenning-innovatiebudget-2025/"
                >Innovatiebudget 2025</a
              >
              van de Digitale Overheid:
            </p>
            <div class="rr-grid">
              <article class="rr-card">
                <span class="rr-card-meta">In samenwerking met VNG</span>
                <h3>Minder burgers in de knel door machineleesbare wetgeving</h3>
                <p>
                  Hoe kunnen we voorkomen dat de stapeling van wet- en
                  regelgeving wetten onuitvoerbaar maakt? Dit project verkent de
                  ontwikkeling van een analysetool om wetsvoorstellen te toetsen
                  op uitvoerbaarheid in samenhang met andere wetten.
                </p>
              </article>
              <article class="rr-card">
                <span class="rr-card-meta">In samenwerking met Dienst Toeslagen</span>
                <h3>Modern rekenhart als bouwsteen voor de hele overheid</h3>
                <p>
                  Kunnen we een algemeen rekenhart voor de overheid ontwikkelen?
                  Dit project verkent hoe zo'n systeem zou kunnen helpen bij het
                  uitvoeren van complexe regelingen voor burgers en bedrijven,
                  bijvoorbeeld bij het berekenen van toeslagen.
                </p>
              </article>
            </div>
          </div>
        </section>

        <section
          id="references"
          class="rr-section rr-section--alt"
          aria-labelledby="rr-refs-title"
        >
          <div class="rr-container">
            <h2 id="rr-refs-title" class="rr-section-title">
              Relevante rapporten en bronnen
            </h2>
            <p class="rr-lede">
              Een overzicht van belangrijke rapporten, onderzoeken en bronnen die
              de noodzaak voor machine-uitvoerbare wetgeving onderbouwen.
            </p>
            <ul class="rr-refs">
              <li v-for="ref in references" :key="ref.title">
                <h3>{{ ref.title }}</h3>
                <p class="rr-ref-meta">{{ ref.meta }}</p>
                <p>{{ ref.text }}</p>
                <a :href="ref.href">
                  {{ ref.linkLabel }}
                  <span class="rr-visually-hidden">— {{ ref.title }}</span>
                </a>
              </li>
            </ul>
          </div>
        </section>

        <section id="faq" class="rr-section" aria-labelledby="rr-faq-title">
          <div class="rr-container">
            <h2 id="rr-faq-title" class="rr-section-title">
              Vragen bij deze verkenning
            </h2>
            <div class="rr-faq">
              <details v-for="item in faq" :key="item.q">
                <summary>{{ item.q }}</summary>
                <p>{{ item.a }}</p>
              </details>
            </div>
          </div>
        </section>

        <section
          id="feedback"
          class="rr-section rr-section--alt rr-feedback"
          aria-labelledby="rr-feedback-title"
        >
          <div class="rr-container">
            <h2 id="rr-feedback-title" class="rr-section-title">Wat denk jij?</h2>
            <p>
              Deze verkenning van machine-uitvoerbare wetgeving roept veel vragen
              op. Hoe zie jij de toekomst van de digitale overheid? Wat zijn je
              zorgen en verwachtingen bij deze ontwikkelingen? Jouw input helpt
              ons deze verkenning verder vorm te geven.
            </p>
            <p>
              <a class="rr-btn rr-btn--primary" href="/nl/aanmelden">
                Meld je aan of deel je gedachten
              </a>
            </p>
          </div>
        </section>
      </template>
    </main>

    <RrFooter />
  </div>
</template>

<script lang="ts">
export default {
  data() {
    return {
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
      steps: [
        {
          title: 'Analoog naar digitaal',
          text:
            'Kunnen bestaande wetten systematisch worden omgezet van analoge tekst naar machine-uitvoerbare specificaties? Een eerste stap om een digitale basis te onderzoeken.',
        },
        {
          title: 'Digitaal rechtsstelsel',
          text:
            'Zouden nieuwe wetten vanaf het begin machine-uitvoerbaar kunnen worden geschreven? We verkennen hoe dat eruit zou kunnen zien en hoe we daarin kunnen ondersteunen.',
        },
        {
          title: 'Gestandaardiseerd ecosysteem',
          text:
            'Een landelijke infrastructuur waar alle overheidssystemen dezelfde wettelijke definities gebruiken. Eén bron van waarheid voor regeltoepassing.',
        },
        {
          title: 'Harmonisatie van wetgeving',
          text:
            'Het wordt mogelijk om systematisch te werken aan harmonisatie van bestaande wetgeving. Conflicten en inconsistenties tussen bestaande regelsets kunnen automatisch worden gedetecteerd, waardoor harmonisatie een bewuste keuze wordt.',
        },
        {
          title: 'Uitvoerbaarheidstoets',
          text:
            'Nieuwe wetten kunnen worden getest voordat ze ingaan. Het effect van nieuwe wetgeving op de consistentie van het rechtsstelsel kan tijdens het wetgevingsproces worden geanalyseerd.',
        },
        {
          title: 'Centrale publicatie',
          text:
            'Machine-uitvoerbare wetgeving wordt centraal gepubliceerd voor iedereen. Execution engines worden ook beschikbaar gesteld zodat alle partijen dezelfde wetten op identieke wijze kunnen uitvoeren.',
        },
        {
          title: 'Transparante toepassing',
          text:
            'Burgers en bedrijven kunnen de exacte werking van regels inzien en controleren. Volledige inzichtelijkheid in regeltoepassing.',
        },
      ],
      tools: [
        {
          title: 'NRML-taal',
          link: { href: 'https://github.com/MinBZK/NRML', label: 'GitHub' },
          text:
            'Normalized Rule Model Language — een JSON-gebaseerd uitwisselings- en opslagformaat voor machine-uitvoerbare wetten. NRML maakt complexe juridische logica toegankelijk, ondersteunt meertalige regels en heeft ingebouwde versioning.',
        },
        {
          title: 'Execution engines',
          meta: 'Python • Go • meer komen',
          text:
            'Meerdere execution engines die NRML-code uitvoeren. Verschillende talen voor verschillende use cases — maar consistent juridisch resultaat.',
        },
        {
          title: 'Analoog-recht-converter',
          meta: 'AI-ondersteund',
          text:
            'LLM-gebaseerde tool om bestaand analoog recht om te zetten naar machine-uitvoerbare NRML. Automatische transformatie van traditionele wetgeving naar een digitaal rechtsstelsel.',
        },
        {
          title: 'Visual law editor',
          meta: 'Blockly-gebaseerd',
          text:
            'Intuïtieve drag-and-drop-interface voor juristen om wetten te schrijven zonder programmeerkennis. Van visuele logica naar uitvoerbare code.',
        },
        {
          title: 'Dependency mapper',
          meta: 'Relatie-analyse',
          text:
            'Visualiseert complexe relaties tussen verschillende wetten. Toont hoe wijzigingen in één wet doorwerken in het gehele juridische ecosysteem.',
        },
        {
          title: 'Regelbeheer',
          meta: 'Git-gebaseerd',
          text:
            'Volledige versioning van machine-uitvoerbare wetgeving. Track wijzigingen, rollback-mogelijkheden, en branch verschillende wetsvoorstellen.',
        },
        {
          title: 'Simulatieomgeving',
          link: {
            href: 'https://ui.lac.apps.digilab.network/simulation',
            label: 'Live demo',
          },
          text:
            'Test de gevolgen van nieuwe wetgeving vóór implementatie. Model maatschappelijke impact en voorspel onbedoelde effecten.',
        },
        {
          title: 'Publicatieplatform',
          meta: 'API + web',
          text:
            'Centrale publicatie en distributie van machine-uitvoerbare wetgeving. API-toegang voor alle overheidssystemen en private partijen.',
        },
      ],
      references: [
        {
          title: 'Factsheet digitale uitvoering van wetgeving',
          meta: 'Prof. Corien Prins (WRR) & Prof. Johan Wolswinkel (Tilburg University) • 23 januari 2025',
          text:
            'Deze WRR-factsheet identificeert vijf aandachtspunten en toetsvragen voor parlementaire controle op digitale uitvoering van wetgeving. Het RegelRecht-project valt binnen het domein van deze factsheet en kan worden beoordeeld aan de hand van de voorgestelde criteria voor transparantie, traceerbaarheid en democratische controle.',
          href: 'https://www.wrr.nl/actueel/nieuws/2025/01/23/factsheet-digitale-uitvoering-van-wetgeving',
          linkLabel: 'WRR',
        },
        {
          title:
            "Factsheet rechtsstatelijke risico's van de digitale uitvoering van wetten",
          meta: 'Dr. Mariette Lokin (OU/VU) & prof. mr. dr. Reijer Passchier (OU/Universiteit Leiden) • 29 november 2024',
          text:
            "Deze factsheet voor de Vaste commissie Digitale Zaken van de Tweede Kamer benoemt zes rechtsstatelijke risico's van digitale wetsuitvoering, waaronder ondoorzichtigheid en vertaalproblemen tussen wettekst en code, en bepleit traceerbaarheid van algoritmen naar hun juridische bron.",
          href: 'https://www.eerstekamer.nl/bijlage/20250129/wetenschappelijke_factsheet/document3/f=/vmkgn0uje7le.pdf',
          linkLabel: 'PDF',
        },
        {
          title: 'Informatiehuishouding, de postkoets met hulpmotor',
          meta: 'Arre Zuurmond (Regeringscommissaris) • 1 mei 2023',
          text:
            'Zuurmond signaleert dat de huidige informatiehuishouding een bureaucratische, reactieve overheid ondersteunt die te sterk gebaseerd is op wantrouwen jegens burgers. Hij pleit voor een responsieve overheid met betere informatievoorziening.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2023/05/01/rapportage-regeringscommissaris-informatiehuishouding',
          linkLabel: 'Rapport',
        },
        {
          title: 'Algoritmes getoetst',
          meta: 'Algemene Rekenkamer • 18 mei 2022',
          text:
            "De Rekenkamer testte 9 algoritmes bij verschillende overheidsorganisaties en constateerde dat 6 daarvan risico's hadden op het gebied van prestatiebeheer, bias, datalekken of onbevoegde toegang. Het rapport benadrukt de noodzaak van continue monitoring.",
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2022/05/18/algoritmes-getoetst',
          linkLabel: 'Rapport',
        },
        {
          title: 'Aandacht voor algoritmes',
          meta: 'Algemene Rekenkamer • 26 januari 2021',
          text:
            'Dit eerste systematische onderzoek naar algoritmegebruik bij de Nederlandse overheid constateerde dat algoritmes zich vooral richten op overheidsbehoeften, met beperkte aandacht voor ethische aspecten en burgerinzicht.',
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2021/01/26/aandacht-voor-algoritmes',
          linkLabel: 'Rapport',
        },
        {
          title: 'Aanbevelingen wetgevingsproces en wetgevingskwaliteit',
          meta: 'Raad van State (Afdeling advisering) • 19 april 2021',
          text:
            'De Raad van State benadrukt het belang van uitvoeringstoetsen en samenwerking tussen beleidsmakers, wetgevingsjuristen en uitvoeringsorganisaties in multidisciplinaire teams, en pleit voor betere toetsing op uitvoerbaarheid en doenvermogen.',
          href: 'https://www.raadvanstate.nl/actueel/nieuws/@125178/aanbevelingen-wetgevingsproces',
          linkLabel: 'Aanbevelingen',
        },
        {
          title:
            'Gematigde groei — Staatscommissie Demografische Ontwikkelingen 2050',
          meta: 'Staatscommissie o.v.v. Richard van Zwol • 15 januari 2024',
          text:
            'De staatscommissie signaleert dat demografische ontwikkelingen leiden tot druk op toegankelijkheid van overheidsdiensten zoals onderwijs, zorg en huisvesting.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2024/01/15/gematigde-groei-rapport-van-de-staatscommissie-demografische-ontwikkleingen-2050',
          linkLabel: 'Rapport',
        },
        {
          title: 'Maak waar! — De digitale overheid',
          meta: 'Studiegroep Informatiesamenleving en Overheid (o.v.v. Richard van Zwol) • 18 april 2017',
          text:
            'De studiegroep concludeert dat digitalisering van de overheid een radicale mentaliteitsverandering vereist en dat digitale dienstverlening tot de kern van het primaire proces hoort.',
          href: 'https://kennisopenbaarbestuur.nl/documenten/rapporten/2017/04/18/maak-waar',
          linkLabel: 'Rapport',
        },
        {
          title: 'Werk aan Uitvoering — Fase 2: Handelingsperspectieven',
          meta: 'Interdepartementaal (BZK, Financiën, OCW, SZW) • 3 juli 2020',
          text:
            'Dit rapport analyseert problemen bij uitvoeringsorganisaties zoals de Belastingdienst, DUO en UWV: continuïteitsrisico’s, beperkte wendbaarheid bij beleidswijzigingen, en ontbrekende mogelijkheden voor maatwerk.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2020/07/03/werk-aan-uitvoering-fase-2-handelingsperspectieven-en-samenvatting-analyse',
          linkLabel: 'Rapport',
        },
        {
          title:
            'Open op orde: generiek actieplan informatiehuishouding Rijksoverheid',
          meta: 'Ministerie van BZK • 6 april 2021',
          text:
            "Dit actieplan werd opgesteld als reactie op het rapport 'Ongekend onrecht' en richt zich op structurele verbetering van de informatiehuishouding binnen de gehele rijksoverheid.",
          href: 'https://www.eerstekamer.nl/overig/20210406/open_op_orde_generiek_actieplan/document3/f=/vlhqp2mq5pvc.pdf',
          linkLabel: 'PDF',
        },
      ],
      faq: [
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
    }
  },
}
</script>
