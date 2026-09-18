/**
 * Voorbeelden bij de beleidsassistent, per modus.
 *
 * Waarom dit een eigen bestand is en geen lijstje in de component: dit is
 * inhoud van het dossier, net als regimeFacts.js. Wie de casus kent hoort deze
 * teksten te kunnen aanscherpen zonder een Vue-component open te slaan.
 *
 * Waarom er per modus meerdere staan, en verschillende: één placeholder laat
 * zien dát je iets kunt typen, niet wát de assistent kan. De set per modus dekt
 * daarom met opzet verschillende assen van de casus: een parameter draaien, de
 * regel zelf veranderen, twee posten tegen elkaar afwegen, en wat een wijziging
 * doet voor één debiteur. Een bezoeker die ze naast elkaar ziet, ziet de
 * reikwijdte.
 *
 * `toelichting` is het halve punt: die zegt wát je te zien krijgt, niet wat je
 * intypt. Zo leert de lijst ook hoe de drie modi van elkaar verschillen.
 *
 * Elk voorbeeld is tegen de echte assistent gedraaid; een voorbeeld dat een
 * parameter noemt die niet bestaat is erger dan geen voorbeeld.
 */
export const VOORBEELDEN = {
  vraag: [
    {
      tekst: 'Welk regime pakt het gunstigst uit voor iemand met een laag inkomen en een hoge schuld?',
      toelichting: 'vergelijkt de vier regimes op dezelfde persoon',
    },
    {
      tekst: 'Hoeveel debiteuren onder SF15-oud hebben nooit een draagkrachtmeting aangevraagd, en wat kost hun dat?',
      toelichting: 'rekent de populatie door op één kenmerk',
    },
    {
      tekst: 'Wat is het verschil tussen variant a2 en a3 voor Kwame?',
      toelichting: 'zet twee varianten naast elkaar op één persoon',
    },
    {
      tekst: 'Waarom betaalt Mariska meer dan Priya bij hetzelfde inkomen?',
      toelichting: 'herleidt een uitkomst tot de artikelen die hem veroorzaken',
    },
  ],
  doel: [
    {
      tekst: 'Minimaliseer het aantal debiteuren met betalingsproblemen zonder de kwijtscheldingskosten meer dan te verdubbelen',
      toelichting: 'laat het optimalisatiepad lopen; hier stelt de assistent je een keuze voor',
    },
    {
      tekst: 'Breng het aandeel levenslang-debiteuren omlaag, zonder dat iemand er maandelijks op achteruitgaat',
      toelichting: 'een doel met een harde randvoorwaarde',
    },
    {
      tekst: 'Trek SF15-oud gelijk met SF15-nieuw binnen de huidige kwijtscheldingskosten',
      toelichting: 'gelijktrekken tegen een budget',
    },
  ],
  instructie: [
    {
      tekst: 'Verhoog de draagkrachtvrije voet van SF15-oud naar 84% van het belastbaar minimumloon',
      toelichting: 'één parameter, met de wettekst mee',
    },
    {
      tekst: 'Maak de draagkrachtmeting ambtshalve in plaats van op aanvraag',
      toelichting: 'een structuurwijziging, geen getal',
    },
    {
      tekst: 'Maximeer de aflosfase op 40 jaar bij een partneropt-out',
      toelichting: 'herbouwt variant b vanaf huidig recht',
    },
  ],
};
