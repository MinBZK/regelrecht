// De gegevens van het basiswerk (zie src/content/docs/basiswerk/). Hier bijwerken.
export const schemas = {
  "S-index": {
    "code": "S-index",
    "vorm": "stroom",
    "title": "De opbouw van het werk",
    "stappen": [
      {
        "label": "Deel 0: Het kompas",
        "tijd": "1 hoofdstuk"
      },
      {
        "label": "Deel 1: Constitutionele architectuur",
        "tijd": "3 hoofdstukken"
      },
      {
        "label": "Deel 2: Hoe een norm ontstaat",
        "tijd": "4 hoofdstukken",
        "markering": true
      },
      {
        "label": "Deel 3: Van norm naar burger",
        "tijd": "4 hoofdstukken"
      },
      {
        "label": "Deel 4: Van wet naar uitvoering",
        "tijd": "3 hoofdstukken"
      },
      {
        "label": "Deel 5: Waar het nu beweegt",
        "tijd": "1 hoofdstuk"
      }
    ],
    "onthoud": "grondslag → norm → burger → uitvoering → beweging.",
    "caption": "Het werkblad past het best na deel 2 (gestippeld).",
    "hoofdstuk": "/basiswerk/inleiding",
    "hoofdstukTitel": "Staats- en bestuursrecht voor RegelRecht",
    "deel": null
  },
  "S0": {
    "code": "S0",
    "vorm": "kaart",
    "title": "Het kompas",
    "kolommen": [
      {
        "kop": "Bevoegdheid",
        "vraag": "Waar komt de bevoegdheid vandaan?",
        "items": [
          "Welk voorschrift geeft deze bevoegdheid?",
          "Aan welk orgaan, en handelt dat orgaan hier zelf?",
          "Attributie, delegatie of mandaat?"
        ]
      },
      {
        "kop": "Rechtsvorm",
        "vraag": "Welke rechtsvorm heeft wat hier gebeurt?",
        "items": [
          "Voorschrift, beleidsregel, besluit of feitelijk handelen?",
          "Op welke trede van de regelgevingsladder staat het?",
          "Wat bindt het, en wie?"
        ]
      },
      {
        "kop": "Rechtsbescherming",
        "vraag": "Wie kan er wat tegen doen, en waar?",
        "items": [
          "Is er een besluit? Zonder besluit geen bezwaar.",
          "Wie is hier belanghebbende?",
          "Welke route, welke rechter, welke termijn?"
        ]
      }
    ],
    "onthoud": "drie kolommen: bevoegdheid, vorm, bescherming.",
    "caption": "Een oriëntatiekaart en geen samenvatting: de kolommen krijgen hun inhoud in deel 2 en deel 3.",
    "hoofdstuk": "/basiswerk/kompas",
    "hoofdstukTitel": "Het kompas",
    "deel": "Deel 0"
  },
  "S1": {
    "code": "S1",
    "vorm": "kaart",
    "title": "Actorenkaart van de democratische rechtsstaat",
    "svg": "\n<svg viewBox=\"0 0 900 400\" role=\"img\" aria-labelledby=\"s1-titel s1-uitleg\" class=\"rr-s1\">\n  <title id=\"s1-titel\">Actorenkaart van de democratische rechtsstaat</title>\n  <desc id=\"s1-uitleg\">De kiezer kiest de Staten-Generaal, die de regering controleert. Samen stellen\n  zij de wet vast. Krachtens die wet komen algemene maatregelen van bestuur en ministeriële\n  regelingen tot stand, die een bestuursorgaan toepast in een besluit aan de burger. De burger kan\n  daartegen in beroep bij de rechter. De rechter toetst dat besluit, maar toetst de wet niet aan de\n  Grondwet: die pijl is doorgekruist, artikel 120 Grondwet.</desc>\n\n  <defs>\n    <marker id=\"s1-punt\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\"\n            markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\">\n      <path d=\"M0 0 L10 5 L0 10 z\" />\n    </marker>\n  </defs>\n\n  <g class=\"rr-s1__doos\">\n    <rect x=\"20\" y=\"20\" width=\"150\" height=\"52\" rx=\"6\" />\n    <text x=\"95\" y=\"52\">Kiezer</text>\n    <rect x=\"20\" y=\"120\" width=\"170\" height=\"52\" rx=\"6\" />\n    <text x=\"105\" y=\"152\">Staten-Generaal</text>\n    <rect x=\"280\" y=\"120\" width=\"150\" height=\"52\" rx=\"6\" />\n    <text x=\"355\" y=\"152\">Regering</text>\n    <rect x=\"700\" y=\"120\" width=\"170\" height=\"52\" rx=\"6\" />\n    <text x=\"785\" y=\"152\">Rechter</text>\n    <rect x=\"150\" y=\"230\" width=\"150\" height=\"52\" rx=\"6\" />\n    <text x=\"225\" y=\"262\">De wet</text>\n    <rect x=\"150\" y=\"330\" width=\"150\" height=\"52\" rx=\"6\" />\n    <text x=\"225\" y=\"355\">AMvB en</text>\n    <text x=\"225\" y=\"373\">ministeriële regeling</text>\n    <rect x=\"390\" y=\"330\" width=\"150\" height=\"52\" rx=\"6\" />\n    <text x=\"465\" y=\"355\">Bestuurs-</text>\n    <text x=\"465\" y=\"373\">orgaan</text>\n    <rect x=\"630\" y=\"330\" width=\"150\" height=\"52\" rx=\"6\" />\n    <text x=\"705\" y=\"362\">Burger</text>\n  </g>\n\n  <g class=\"rr-s1__pijl\">\n    <path d=\"M95 72 V114\" />\n    <path d=\"M190 146 H274\" />\n    <path d=\"M120 172 L198 224\" />\n    <path d=\"M330 172 L264 224\" />\n    <path d=\"M225 282 V324\" />\n    <path d=\"M300 356 H384\" />\n    <path d=\"M540 356 H624\" />\n    <path d=\"M705 330 V178\" />\n    <path d=\"M700 168 Q560 250 512 326\" class=\"rr-s1__toets\" />\n  </g>\n\n  <g class=\"rr-s1__verboden\">\n    <path d=\"M700 150 L308 244\" class=\"rr-s1__verbodenpijl\" />\n    <path d=\"M410 207 L430 227\" class=\"rr-s1__kruis\" />\n    <path d=\"M430 207 L410 227\" class=\"rr-s1__kruis\" />\n  </g>\n\n  <g class=\"rr-s1__label\">\n    <text x=\"105\" y=\"100\">kiest</text>\n    <text x=\"232\" y=\"138\">controleert</text>\n    <text x=\"225\" y=\"212\">stellen samen vast</text>\n    <text x=\"236\" y=\"310\">krachtens de wet</text>\n    <text x=\"342\" y=\"348\">past toe</text>\n    <text x=\"582\" y=\"348\">besluit</text>\n    <text x=\"716\" y=\"258\">beroep</text>\n    <text x=\"612\" y=\"286\" class=\"rr-s1__label--toets\">toetst het besluit</text>\n    <text x=\"583\" y=\"222\" class=\"rr-s1__label--verboden\">toetst niet aan de Grondwet (art. 120)</text>\n  </g>\n</svg>\n        ",
    "onthoud": "één keten van kiezer naar burger, en één doorgestreepte pijl: de rechter toetst alles behalve de wet aan de Grondwet.",
    "caption": "De doorgestreepte pijl is de haak waaraan deel 5 hangt. Toetsing van diezelfde wet aan een verdrag mag wél, op grond van artikel 94 Grondwet.",
    "hoofdstuk": "/basiswerk/rechtsstaat-democratie",
    "hoofdstukTitel": "Rechtsstaat en democratie als spanning",
    "deel": "Deel 1",
    "regelrecht": [
      {
        "tekst": "Nu kan alleen de uitvoerende macht nagaan wat haar systemen met de wet doen. Een gepubliceerde specificatie zou de rechter, het parlement en de burger elk binnen hun eigen ambt laten nagaan wat nu alleen de uitvoerder kan.",
        "paper": "sec:executivedominance"
      }
    ]
  },
  "S2": {
    "code": "S2",
    "vorm": "kaart",
    "title": "Drie overlappende functies",
    "svg": "\n<svg viewBox=\"0 0 820 520\" role=\"img\" aria-labelledby=\"s2-titel s2-uitleg\" class=\"rr-s2\">\n  <title id=\"s2-titel\">Drie overlappende functies</title>\n  <desc id=\"s2-uitleg\">Drie elkaar overlappende cirkels: wetgeving, bestuur en rechtspraak. In de\n  overlap van wetgeving en bestuur staat de regering als medewetgever. In de overlap van wetgeving\n  en rechtspraak staat de Raad van State, die adviseert over wetgeving en rechtspreekt. In de\n  overlap van bestuur en rechtspraak staat het bezwaar, waarbij het orgaan zijn eigen besluit\n  beoordeelt. In het midden, waar alle drie samenkomen, staat de open norm.</desc>\n\n  <g class=\"rr-s2__cirkel\">\n    <circle cx=\"310\" cy=\"200\" r=\"160\" />\n    <circle cx=\"510\" cy=\"200\" r=\"160\" />\n    <circle cx=\"410\" cy=\"350\" r=\"160\" />\n  </g>\n\n  <g class=\"rr-s2__naam\">\n    <text x=\"215\" y=\"105\">Wetgeving</text>\n    <text x=\"605\" y=\"105\">Bestuur</text>\n    <text x=\"410\" y=\"480\">Rechtspraak</text>\n  </g>\n\n  <g class=\"rr-s2__overlap\">\n    <text x=\"410\" y=\"158\">regering als</text>\n    <text x=\"410\" y=\"176\">medewetgever</text>\n\n    <text x=\"292\" y=\"300\">Raad van</text>\n    <text x=\"292\" y=\"318\">State</text>\n\n    <text x=\"528\" y=\"300\">bezwaar bij het</text>\n    <text x=\"528\" y=\"318\">orgaan zelf</text>\n\n    <text x=\"410\" y=\"258\">de open norm</text>\n  </g>\n</svg>\n        ",
    "onthoud": "drie cirkels die elkaar raken, en in elke overlap één ambt dat twee dingen tegelijk doet.",
    "caption": "De overlappen zijn de les; de cirkels zijn slechts de aanleiding. In het midden staat de open norm, omdat die door alle drie de functies wordt aangeraakt: de wetgever laat hem open, het bestuur vult hem in, de rechter toetst die invulling.",
    "hoofdstuk": "/basiswerk/machtenscheiding",
    "hoofdstukTitel": "Machtenscheiding is overlap, geen scheiding",
    "deel": "Deel 1"
  },
  "S3": {
    "code": "S3",
    "vorm": "stroom",
    "title": "Twee sporen van grondrechtenbescherming",
    "stappen": [
      {
        "label": "Is er een beperking?"
      },
      {
        "label": "Is er een beperkingsclausule?"
      },
      {
        "label": "Welke regelingsvorm eist die?"
      },
      {
        "label": "Is delegatie toegestaan?"
      }
    ],
    "varianten": [
      {
        "label": "Naast dit spoor loopt de toets van het EVRM",
        "stappen": [
          {
            "label": "Bij wet voorzien"
          },
          {
            "label": "Legitiem doel"
          },
          {
            "label": "Noodzakelijk in een democratische samenleving"
          }
        ]
      }
    ],
    "onthoud": "twee banen naast elkaar: de Nederlandse beperkingssystematiek boven, de EVRM-toets eronder.",
    "caption": "De parallelle weergave ís de les: hetzelfde feitencomplex wordt langs twee maatstaven gelegd, en die kunnen uiteenlopen.",
    "hoofdstuk": "/basiswerk/grondrechten",
    "hoofdstukTitel": "Grondrechten en hun beperking",
    "deel": "Deel 1"
  },
  "S4": {
    "code": "S4",
    "vorm": "kaart",
    "title": "Gelaagdheid van regelgeving",
    "ladder": {
      "kop": [
        "vastgesteld door",
        "bekendmaking",
        "toetst de rechter?",
        "vindplaats"
      ],
      "treden": [
        {
          "duo": true,
          "blokken": [
            {
              "label": "Statuut en Grondwet",
              "cellen": [
                "grondwetgever: twee lezingen, de tweede met tweederde meerderheid",
                "Staatsblad",
                "hoogste nationale recht, maar de rechter mag wetten en verdragen er niet aan toetsen (art. 120 Gw)",
                "wetten.overheid.nl; denederlandsegrondwet.nl"
              ]
            },
            {
              "label": "EU-recht en een ieder verbindende verdragsbepalingen",
              "cellen": [
                "Europese wetgever; verdragspartijen",
                "Publicatieblad van de EU; Tractatenblad",
                "de rechter moet strijdig nationaal recht buiten toepassing laten (art. 94 Gw)",
                "eur-lex.europa.eu; de verdragenbank"
              ]
            }
          ],
          "verhouding": "Twee ordes op dezelfde hoogte, zonder rangorde onderling, en asymmetrisch. Verdragsbepalingen met rechtstreekse werking zetten nationale voorschriften opzij, naar heersende opvatting ook de Grondwet zelf; omgekeerd mag de rechter een verdrag niet aan de Grondwet toetsen. Voor EU-recht komt daar nog bij dat het Hof van Justitie de voorrang als eigen leerstuk beschouwt, los van wat een nationale grondwet bepaalt."
        },
        {
          "label": "Wet in formele zin",
          "cellen": [
            "regering en Staten-Generaal samen",
            "Staatsblad",
            "niet aan de Grondwet (art. 120 Gw); wel aan verdragen",
            "wetten.overheid.nl"
          ]
        },
        {
          "label": "Algemene maatregel van bestuur",
          "cellen": [
            "regering, bij koninklijk besluit, na verplicht advies van de Raad van State",
            "Staatsblad",
            "ja, exceptieve toetsing aan hoger recht",
            "wetten.overheid.nl"
          ]
        },
        {
          "label": "Ministeriële regeling",
          "cellen": [
            "één minister",
            "Staatscourant",
            "ja, exceptieve toetsing aan hoger recht",
            "wetten.overheid.nl"
          ]
        },
        {
          "label": "Decentrale verordening",
          "cellen": [
            "gemeenteraad, provinciale staten, algemeen bestuur van het waterschap",
            "gemeenteblad, provinciaal blad, waterschapsblad",
            "ja, exceptieve toetsing aan hoger recht",
            "lokale regelgeving op overheid.nl"
          ]
        }
      ]
    },
    "ernaast": {
      "kop": "Náást de ladder, niet erin",
      "vraag": "beleidsregels en interne regels",
      "items": [
        "Vastgesteld door het bestuursorgaan zelf, over hoe het zijn eigen bevoegdheid gebruikt (art. 4:81 Awb).",
        "Geen algemeen verbindend voorschrift: het bindt de burger niet rechtstreeks (art. 1:3 lid 4 Awb).",
        "Bindt wél het orgaan, inclusief de plicht om af te wijken bij bijzondere omstandigheden (art. 4:84 Awb).",
        "Een interne regel werkt niet naar buiten: hij stuurt de organisatie, niet de burger (aanwijzing 2.17)."
      ]
    },
    "onthoud": "een duo bovenaan, vier treden eronder, en één kader ernaast: drie richtingen, geen rechte lijn.",
    "caption": "Binnen het duo geldt formeel nog dat het Statuut voorgaat op de Grondwet; voor het dagelijks werk in de uitvoering valt dat samen. Bij de zorgtoeslag staan de aanspraak en de percentages in de wet, mogen de percentages bij AMvB wijzigen, en komt de standaardpremie uit een ministeriële regeling.",
    "hoofdstuk": "/basiswerk/gelaagdheid",
    "hoofdstukTitel": "De gelaagdheid van regelgeving",
    "deel": "Deel 2"
  },
  "S5": {
    "code": "S5",
    "vorm": "matrix",
    "title": "Attributie, delegatie, mandaat",
    "kop": [
      "Attributie",
      "Delegatie",
      "Mandaat"
    ],
    "rijen": [
      {
        "label": "Wie krijgt de bevoegdheid",
        "cellen": [
          "een orgaan krijgt een nieuwe, eigen bevoegdheid",
          "een ander orgaan krijgt een bestaande bevoegdheid overgedragen",
          "niemand; de bevoegdheid blijft waar hij is"
        ]
      },
      {
        "label": "In wiens naam wordt gehandeld",
        "cellen": [
          "eigen naam",
          "eigen naam",
          "naam van de mandaatgever"
        ]
      },
      {
        "label": "Waar ligt de verantwoordelijkheid",
        "cellen": [
          "bij het orgaan zelf",
          "bij het orgaan dat de bevoegdheid kreeg",
          "bij de mandaatgever"
        ]
      }
    ],
    "onthoud": "drie kolommen, drie rijen: wie krijgt hem, in wiens naam, wie draagt hem.",
    "hoofdstuk": "/basiswerk/bevoegdheid",
    "hoofdstukTitel": "Waar bevoegdheid vandaan komt",
    "deel": "Deel 2"
  },
  "S6": {
    "code": "S6",
    "vorm": "stroom",
    "title": "De weg van een wetsvoorstel",
    "stappen": [
      {
        "label": "Probleemsignalering"
      },
      {
        "label": "Afweging via het Beleidskompas"
      },
      {
        "label": "Voorontwerp"
      },
      {
        "label": "Internetconsultatie",
        "extern": true
      },
      {
        "label": "Ministerraad"
      },
      {
        "label": "Advies Afdeling advisering"
      },
      {
        "label": "Indiening Tweede Kamer"
      },
      {
        "label": "Schriftelijke behandeling en amendering",
        "extern": true
      },
      {
        "label": "Stemming Tweede Kamer"
      },
      {
        "label": "Eerste Kamer",
        "tijd": "geen recht van amendement",
        "markering": true
      },
      {
        "label": "Bekrachtiging"
      },
      {
        "label": "Staatsblad"
      },
      {
        "label": "Inwerkingtreding",
        "tijd": "datum uit de wet of bij koninklijk besluit"
      }
    ],
    "varianten": [
      {
        "label": "Dezelfde weg voor een algemene maatregel van bestuur",
        "stappen": [
          {
            "label": "Voorontwerp"
          },
          {
            "label": "Consultatie"
          },
          {
            "label": "Voorhang, als de wet die eist",
            "extern": true
          },
          {
            "label": "Ministerraad"
          },
          {
            "label": "Advies Afdeling advisering"
          },
          {
            "label": "Koninklijk besluit"
          },
          {
            "label": "Staatsblad"
          }
        ]
      },
      {
        "label": "En voor een ministeriële regeling",
        "stappen": [
          {
            "label": "Ontwerp"
          },
          {
            "label": "Vaststelling door de minister"
          },
          {
            "label": "Staatscourant"
          }
        ]
      }
    ],
    "onthoud": "één lange rij met twee kortere eronder: hoe lager de regeling, hoe korter de weg.",
    "caption": "Het contrast tussen de drie rijen is de les. De doorlooptijd verschilt sterk per dossier; de gemarkeerde stappen zijn de plekken waar iemand van buiten de tekst nog kan beïnvloeden.",
    "hoofdstuk": "/basiswerk/wetgevingsproces",
    "hoofdstukTitel": "Het wetgevingsproces",
    "deel": "Deel 2",
    "regelrecht": [
      {
        "stap": "Stemming Tweede Kamer",
        "tekst": "Vóór de stemming kan een specificatie de rekenvoorbeelden uit de memorie van toelichting naspelen, en een voorgesteld amendement doorrekenen.",
        "paper": "sec:lawmaking"
      }
    ]
  },
  "S7": {
    "code": "S7",
    "vorm": "stroom",
    "title": "Kwalificatie van een regel",
    "stappen": [
      {
        "label": "Extern bindend?"
      },
      {
        "label": "Algemeen geformuleerd?"
      },
      {
        "label": "Van een orgaan met regelgevende bevoegdheid?"
      }
    ],
    "onthoud": "drie vragen, vier uitkomsten.",
    "caption": "De vier uitkomsten: algemeen verbindend voorschrift, beleidsregel, interne regel, of feitelijk handelen. Een regel voor één geval is geen regel maar een beschikking; die komt in deel 3.",
    "stop": "de kwalificatie is in de praktijk vaak zelf onderwerp van geschil: wat een regel is, hangt af van wat hij doet, niet van het etiket.",
    "hoofdstuk": "/basiswerk/kwalificatie",
    "hoofdstukTitel": "Wat voor soort regel is dit?",
    "deel": "Deel 2"
  },
  "S8": {
    "code": "S8",
    "vorm": "stroom",
    "title": "Van wettelijke bepaling naar rechter",
    "stappen": [
      {
        "label": "Wettelijke bepaling"
      },
      {
        "label": "Bevoegdheid van het bestuursorgaan"
      },
      {
        "label": "Beleidsregel"
      },
      {
        "label": "Beschikking in het individuele geval"
      },
      {
        "label": "Bezwaar"
      },
      {
        "label": "Beroep"
      },
      {
        "label": "Hoger beroep"
      }
    ],
    "onthoud": "zeven stappen, van bepaling naar rechter: de keten die elke regeling doorloopt.",
    "caption": "Leeg is dit schema het werkblad: één echte regeling door deze zeven stappen trekken laat zien waar de beleidsregel zit en waar de ruimte ligt.",
    "hoofdstuk": "/basiswerk/besluit",
    "hoofdstukTitel": "Het besluit als scharnier",
    "deel": "Deel 3",
    "regelrecht": [
      {
        "stap": "Wettelijke bepaling",
        "tekst": "De specificatie is een interpretatie van deze bepaling. Bij strijd gaat de wet voor en wordt de specificatie gecorrigeerd.",
        "paper": "sec:legalstatus"
      },
      {
        "stap": "Beleidsregel",
        "tekst": "De keuzes van de uitvoerder voor grensgevallen, ook die nu niet als beleidsregel worden gepubliceerd maar in werkinstructies staan (§3.2), komen in de gepubliceerde specificatie, waar anderen ze kunnen zien en betwisten. De koppeling met de beleidsregel legt dit basiswerk zelf.",
        "paper": "sec:whochooses"
      },
      {
        "stap": "Beschikking in het individuele geval",
        "tekst": "In het voorstel legt het besluit vast welke versie van de specificatie draaide en op welke invoer. Wie het ontvangt, kan de zaak opnieuw doorrekenen, mits hij dat uitvoeringsspoor ook krijgt; of het huidige recht dat regelt, is volgens de paper onzeker (§4.5).",
        "paper": "sec:attestation"
      },
      {
        "stap": "Beroep",
        "tekst": "De rechter kan de specificatie op de feiten van de zaak draaien en vaststellen of een uitkomst uit de vastlegging komt of uit de wet zelf. Hij mag de specificatie gebruiken en is er niet aan gebonden.",
        "paper": "sec:court"
      }
    ]
  },
  "S10": {
    "code": "S10",
    "vorm": "spectrum",
    "title": "Het discretiespectrum",
    "assen": [
      {
        "label": "De bevoegdheid",
        "links": "gebonden: de uitkomst staat vast",
        "rechts": "beoordelings- en beleidsruimte"
      },
      {
        "label": "De rechterlijke toetsing beweegt mee",
        "links": "vol",
        "rechts": "terughoudender, maar niet afwezig"
      }
    ],
    "punten": [
      {
        "label": "Volledig berekenbaar",
        "tekst": "alleen aan de linkerkant. Naar rechts toe wordt vertalen naar een model steeds meer een keuze in plaats van een afleiding."
      },
      {
        "label": "Geen scherpe grens",
        "tekst": "de meeste bevoegdheden zitten ergens in het midden, en dezelfde bepaling kan per element verschillen."
      }
    ],
    "onthoud": "twee assen die meebewegen: hoe meer ruimte voor het bestuur, hoe terughoudender de rechter, al is dat sinds 2022 aan het verschuiven.",
    "hoofdstuk": "/basiswerk/discretie",
    "hoofdstukTitel": "Waar het beleid in de wet zit",
    "deel": "Deel 3"
  },
  "S11": {
    "code": "S11",
    "vorm": "stroom",
    "title": "De drietrap",
    "stappen": [
      {
        "label": "Geschiktheid",
        "tijd": "draagt de maatregel bij aan het doel?"
      },
      {
        "label": "Noodzakelijkheid",
        "tijd": "kan het ook minder ingrijpend?"
      },
      {
        "label": "Evenwichtigheid",
        "tijd": "staat het gevolg in verhouding tot het doel?"
      }
    ],
    "onthoud": "drie treden, en daarnaast de schuif die bepaalt hoe streng er wordt gekeken.",
    "caption": "De toetsingsintensiteit loopt op naarmate de belangen zwaarder wegen, de gevolgen ernstiger zijn, of er een fundamenteel recht in het geding is.",
    "hoofdstuk": "/basiswerk/evenredigheid",
    "hoofdstukTitel": "De evenredigheidstoets",
    "deel": "Deel 3",
    "regelrecht": [
      {
        "stap": "Noodzakelijkheid",
        "tekst": "Bij een vastgelegde regel kan vóór invoering worden nagegaan of een minder ingrijpende weg naar dezelfde uitkomst bestaat. De afweging in het concrete geval blijft bij bestuur en rechter.",
        "paper": "sec:proportionality"
      }
    ]
  },
  "S12": {
    "code": "S12",
    "vorm": "matrix",
    "title": "Sanctietypen en waarborgen",
    "kop": [
      "Herstelsanctie",
      "Bestraffende sanctie"
    ],
    "rijen": [
      {
        "label": "Doel",
        "cellen": [
          "de overtreding beëindigen of ongedaan maken",
          "leedtoevoeging: straffen voor wat gebeurd is"
        ]
      },
      {
        "label": "Voorbeelden",
        "cellen": [
          "last onder bestuursdwang, last onder dwangsom",
          "bestuurlijke boete"
        ]
      },
      {
        "label": "Waarborgen",
        "cellen": [
          "de algemene beginselen van behoorlijk bestuur",
          "daarbovenop: verwijtbaarheid als voorwaarde, zwijgrecht en de mededeling daarvan"
        ]
      },
      {
        "label": "Hoe streng toetst de rechter",
        "cellen": [
          "op bevoegdheid en evenredigheid, ook van de hoogte van een dwangsom",
          "ook op de hoogte van de boete, afgestemd op ernst en verwijtbaarheid"
        ]
      }
    ],
    "onthoud": "twee kolommen (herstellen of straffen) en vier rijen die verklaren waarom dat verschil ertoe doet.",
    "hoofdstuk": "/basiswerk/handhaving",
    "hoofdstukTitel": "Handhaving en sancties",
    "deel": "Deel 3"
  },
  "S15": {
    "code": "S15",
    "vorm": "matrix",
    "title": "Wie kan wat nagaan",
    "kop": [
      "Nu",
      "Met een gepubliceerde specificatie"
    ],
    "rijen": [
      {
        "label": "Wie een besluit krijgt",
        "cellen": [
          "krijgt de uitkomst, niet de regel zoals die werd uitgevoerd",
          "kan de eigen zaak vooraf en achteraf doorrekenen"
        ]
      },
      {
        "label": "De rechter",
        "cellen": [
          "neemt de uitleg van de uitvoerder aan, of reconstrueert de beslislogica",
          "kan de specificatie op de feiten van de zaak draaien; artikel 120 blijft gelden"
        ]
      },
      {
        "label": "Controleurs met toegang tot de gegevens",
        "cellen": [
          "zien wat de uitvoerder laat zien",
          "kunnen de regel over een hele populatie doorrekenen"
        ]
      },
      {
        "label": "Parlement en publiek",
        "cellen": [
          "lezen de wettekst, niet de regel zoals die wordt uitgevoerd",
          "kunnen de regel zelf lezen en analyseren, zonder zaakgegevens"
        ]
      }
    ],
    "onthoud": "van wat de uitvoerder laat zien, naar wat elke actor binnen zijn eigen ambt kan nagaan.",
    "caption": "Dit is het voorstel uit Rules as Executed (samenvatting en paragrafen 3 en 6), niet de huidige praktijk.",
    "hoofdstuk": "/basiswerk/wat-regelrecht-verandert",
    "hoofdstukTitel": "Wat RegelRecht verandert, en wat niet",
    "deel": "Deel 4"
  },
  "S14": {
    "code": "S14",
    "vorm": "matrix",
    "title": "Wat mag de rechter waaraan toetsen?",
    "kop": [
      "Grondwet",
      "Verdrag en EU-recht",
      "Hogere nationale regeling",
      "Beginselen van behoorlijk bestuur"
    ],
    "rijen": [
      {
        "label": "Wet in formele zin",
        "cellen": [
          null,
          "ja",
          "n.v.t.",
          "n.v.t."
        ]
      },
      {
        "label": "AMvB en ministeriële regeling",
        "cellen": [
          "ja",
          "ja",
          "ja",
          "ja"
        ]
      },
      {
        "label": "Beleidsregel",
        "cellen": [
          "ja",
          "ja",
          "ja",
          "ja"
        ]
      },
      {
        "label": "Besluit in een individueel geval",
        "cellen": [
          "ja",
          "ja",
          "ja",
          "ja"
        ]
      }
    ],
    "onthoud": "één dichte cel: de formele wet tegen de Grondwet. Al het andere staat open.",
    "caption": "De huidige situatie. Direct hieronder staat dezelfde matrix zoals hij onder het wetsvoorstel van 2026 zou worden.",
    "stop": "de discussie hierover loopt; dit beeld heeft een houdbaarheidsdatum.",
    "hoofdstuk": "/basiswerk/constitutionele-toetsing",
    "hoofdstukTitel": "Constitutionele toetsing",
    "deel": "Deel 5",
    "regelrecht": [
      {
        "rij": "Wet in formele zin",
        "tekst": "Publicatie verandert niet wat de rechter mag toetsen. Een harde uitkomst die de wet afdwingt, wordt wel zichtbaar en telbaar voor de wetgever.",
        "paper": "sec:court"
      }
    ]
  },
  "S13": {
    "code": "S13",
    "vorm": "kaart",
    "title": "Welke rechter waarvoor",
    "kolommen": [
      {
        "kop": "Afdeling bestuursrechtspraak",
        "vraag": "Raad van State",
        "items": [
          "de algemene hoogste bestuursrechter",
          "omgevingsrecht",
          "vreemdelingenzaken",
          "toeslagen"
        ]
      },
      {
        "kop": "Centrale Raad van Beroep",
        "vraag": "sociale zekerheid",
        "items": [
          "uitkeringen en voorzieningen",
          "ambtenarenzaken"
        ]
      },
      {
        "kop": "College van Beroep voor het bedrijfsleven",
        "vraag": "economisch bestuursrecht",
        "items": [
          "markttoezicht",
          "landbouw en subsidies"
        ]
      },
      {
        "kop": "Belastingrechter",
        "vraag": "rechtbank, gerechtshof, Hoge Raad",
        "items": [
          "rijksbelastingen",
          "lokale heffingen"
        ]
      },
      {
        "kop": "Civiele rechter",
        "vraag": "de restrechter",
        "items": [
          "als geen bestuursrechtelijke weg openstaat",
          "onrechtmatige daad door de overheid"
        ]
      }
    ],
    "onthoud": "vier hoogste bestuursrechters, naar onderwerp, en de civiele rechter als vangnet.",
    "caption": "Ingang is het type zaak, niet het orgaan. Voor de hoogste bestuursrechters gaan bezwaar en beroep bij de rechtbank voorop. Toeslagen eindigen bij de Afdeling bestuursrechtspraak.",
    "hoofdstuk": "/basiswerk/welke-rechter",
    "hoofdstukTitel": "Welke rechter waarvoor",
    "deel": "Deel 5"
  }
}
