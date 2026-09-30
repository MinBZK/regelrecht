/**
 * Voorbeelden bij de beleidsassistent, per modus.
 *
 * Waarom dit een eigen bestand is en geen lijstje in de component: dit is
 * inhoud van het dossier, net als nieuwkomerFacts.js. Wie de casus kent hoort
 * deze teksten te kunnen aanscherpen zonder een Vue-component open te slaan.
 *
 * Waarom er per modus meerdere staan, en verschillende: één placeholder laat
 * zien dát je iets kunt typen, niet wát de assistent kan. De set per modus dekt
 * daarom met opzet verschillende assen van de casus: een bedrag draaien, de
 * systematiek zelf veranderen, regeling tegen uitvoeringslast afwegen, en wat
 * een wijziging doet voor één leerling. Een bezoeker die ze naast elkaar ziet,
 * ziet de reikwijdte.
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
      tekst: 'Welke variant van de regelingen is voor scholen het meest voordelig?',
      toelichting: 'de vraag van OCW; de assistent zegt erbij voor wie',
    },
    {
      tekst: 'Wat kost de drempel van vier nieuwkomers aan gemiste bekostiging, en bij hoeveel scholen?',
      toelichting: 'rekent een grens door op de populatie',
    },
    {
      tekst: 'Waarom telt Amina wel mee en haar klasgenoot niet?',
      toelichting: 'herleidt de categorie tot de artikelen',
    },
    {
      tekst: 'Hoeveel uur besteden scholen en DUO samen aan één aanvraagronde?',
      toelichting: 'leest het uitvoeringslastmodel',
    },
  ],
  doel: [
    {
      tekst: 'Trek de bedragen voor asielzoekers en overige vreemdelingen in het po gelijk zonder dat de totale uitgave stijgt',
      toelichting: 'gelijktrekken binnen het budget; hier stelt de assistent je een keuze voor',
    },
    {
      tekst: 'Laat het po de systematiek van het vo volgen zonder dat de uitvoeringslast bij scholen stijgt',
      toelichting: 'twee posten tegen elkaar',
    },
    {
      tekst: 'Maak de regeling ambtshalve, en houd de uitgaven binnen een procent van nu',
      toelichting: 'structuur en budget tegelijk',
    },
  ],
  instructie: [
    {
      tekst: 'Laat de drempel van vier nieuwkomers per school in artikel 34 vervallen',
      toelichting: 'één parameter, met de wettekst mee',
    },
    {
      tekst: 'Trek het bedrag voor overige vreemdelingen op naar dat van asielzoekers',
      toelichting: 'één bedrag, groot effect',
    },
    {
      tekst: 'Haal de accountantscontrole uit de po-handelingen',
      toelichting: 'raakt het uitvoeringslastmodel, niet de regeling',
    },
  ],
};
