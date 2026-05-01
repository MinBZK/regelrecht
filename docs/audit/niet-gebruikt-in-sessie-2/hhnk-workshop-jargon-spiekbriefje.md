# Jargon-spiekbriefje — HHNK-expert-workshop

*Houd deze naast je tijdens de sessie. Alle schema-/engine-termen in mensentaal, met één concreet HHNK-voorbeeld per stuk.*

## Kern-metafoor (1 zin voor deelnemers)

> *"Stel je een junior-medewerker voor die je wilt leren kwijtschelding beoordelen. Je geeft haar een stappenplan. De YAML is dat stappenplan. De engine is de medewerker die het uitvoert. Alles wat we vandaag bespreken gaat over: klopt het stappenplan?"*

---

## Snel-overzicht (spiekbriefje)

| Jargon | Mensentaal |
|---|---|
| **YAML** | Het regel-stappenplan als tekstbestand |
| **Engine** | De 'junior-medewerker' die het stappenplan uitvoert |
| **Output** | Het eindantwoord van een wet |
| **Parameter** | Getal dat caller aanlevert (BSN, aanslag) |
| **Input** | Getal dat engine zelf ergens ophaalt (vermogen) |
| **Source** | Mechanisme waarmee input uit andere wet wordt opgehaald |
| **Caller** | Systeem dat de engine aanroept (HHNK-belastingsysteem) |
| **Action** | Eén formule die één output berekent |
| **Definition** | Vaste waarde uit de wet (drempel €500) |
| **Untranslatable** | Wettekst die bewust niet in formule zit (handmatig oordeel) |
| **Override** | Andere wet vervangt jouw formule (Leidraad 2008 → URI auto-drempel) |
| **legal_basis** | Juridische grondslag onder een regel |

---

## Uitgebreide uitleg

### YAML — het stappenplan

Tekstbestand met regels die zowel mens als computer kan lezen. Elke wet heeft eigen YAML-file.

### Engine — de uitvoerder

Het programma dat de YAML leest en tot een antwoord komt.
*"De engine leest de YAML en rekent uit."*

### Output — het eindantwoord

Wat een wet teruggeeft.
*Voorbeeld HHNK-leidraad 26*: `beleidsregel_vereist_verzoek`, `uitgesloten_van_kwijtschelding`, `kan_kwijtschelding_worden_verleend`, `hoogte_kwijtschelding`.

---

## De drie soorten invoer — hier zit vraag F6

### Parameter — wat caller aanlevert

Iets dat de aanroeper zelf meegeeft.
*Voorbeeld*: BSN, aanslagbedrag, huishoudtype. De HHNK-ambtenaar (of het HHNK-systeem) levert deze aan bij de aanvraag.

### Input — wat de engine ophaalt

Waarde die de wet **automatisch** uit een andere wet ophaalt.
*Voorbeeld*: `vermogen_bedrag` is een input in HHNK-leidraad 26. Wordt opgehaald uit URI art 12. Je ziet het niet als aanroeper — engine regelt het.

### Source — het ophaal-mechanisme

*"Deze input `uri_hoogte_kwijtschelding` heeft een source naar URI art 11. Betekent: de engine belt URI art 11 met onze gegevens en gebruikt dat antwoord hier."*

### Caller — wie aanroept

De aanroeper van de engine.
*"Bij HHNK is de caller straks het HHNK-belastingsysteem, dat voor elke kwijtscheldingsaanvraag de engine aanroept met BSN en aanslag."*

### F6-uitleg in 2 zinnen voor deelnemers

> *"Bedragen zoals bijstandsnormen kunnen op twee manieren bij onze formule komen. Óf: jullie HHNK-systeem stopt het getal erin — dat heet 'caller-parameter'. Óf: de engine gaat automatisch naar de Participatiewet-YAML, leest daar het actuele bedrag, en gebruikt dat — dat heet 'source'. Source is mechanisch consistent maar alleen als de Participatiewet ook onderhouden wordt. Caller geeft jullie controle."*

---

## Formules en constanten

### Action — één berekening

Eén regel/formule die één output berekent.
*Voorbeeld*: `uitgesloten_van_kwijtschelding = OR van g₁ₐ, g₁ᵦ, ..., g₇`. Eén action = één uitkomst.

### Definition — vaste waarde

Een vaste waarde uit de wettekst (constante).
*Voorbeeld*: `normpremie_zvw_alleenstaand_maand_2026 = 4700 eurocent` (€47). Staat in 26.2.19 van de leidraad, is een hardcoded bedrag.

### Definition vs parameter — verschil in één zin

> *"Definition = een getal uit de wet zelf. Parameter = een getal dat jullie moeten aanleveren. Drempel van €500 is een definition want die staat in de wet. BSN is een parameter want die verschilt per aanvraag."*

---

## Untranslatable — belangrijk concept

Stuk van de wet dat bewust **niet** in een formule is vertaald.
*Voorbeeld*: *"naar het oordeel van de ontvanger"* uit 26.1.9. Kan niet als formule — vergt menselijke beoordeling per geval.

### Uitleg voor deelnemers

> *"Sommige stukken van de wet kunnen we niet in een formule gieten. 'Naar het oordeel van de ontvanger', 'bijzondere omstandigheden', 'aannemelijk maken' — dat vergt menselijk oordeel. Die markeren we als 'untranslatable'. De engine doet die check niet; de ontvanger doet die handmatig."*

---

## Override — eenmaal in onze keten

Andere wet zegt "gebruik MIJN formule voor jouw output".
*Enige voorbeeld in onze keten*: Leidraad Invordering 2008 art 26.2.3 override op URI art 12's `auto_als_vermogen`. Drempel wordt €3 350 ipv €2 269.

### Uitleg voor deelnemers

> *"URI 1990 zegt: een auto telt als vermogen als die meer dan €2 269 waard is. De Leidraad 2008 zegt: nee, we nemen €3 350. Dat is 'lex pro cive' — gunstiger voor de burger. De engine weet dat: zodra je vermogen berekent, pakt de engine automatisch de Leidraad-drempel ipv de URI-drempel."*

---

## Juridische context-velden

### legal_basis — grondslag

Waar in welke wet berust deze regel op.
*Voorbeeld*: HHNK-leidraad berust op Invorderingswet 1990 art 26, Leidraad 2008 art 26, en HHNK-verordening art 1.
*"Legal basis = waar mag deze wet bestaan? Als je de grondslag wegneemt, verliest de regel z'n kracht."*

### legal_basis.explanation — wettekst-quote per formule

Per formule de letterlijke wettekst-quote die de formule dekt.
*Dit is wat je voorleest uit formulas.md* — *"De YAML zegt: '{quote uit legal_basis.explanation}'. Klopt dat?"*

---

## Regel die altijd werkt in de workshop

Als een deelnemer iets vraagt over een term, **vertaal eerst, leg dan uit**.

*"Is dat een untranslatable dan?"*

Jij: *"Ja, precies — betekent: we hebben 't bewust niet in een formule gestopt omdat het menselijke beoordeling vergt. Bijvoorbeeld '{concreet voorbeeld uit 26.1.9}'."*

Dan weet je zeker dat iedereen bij blijft.

---

## Back-pocket-antwoorden op lastige vragen

**V**: *"Maar wat als jullie formule een getal teruggeeft dat ik in de praktijk niet zou geven?"*
**A**: *"Perfect — dat willen we vandaag weten. Kan je concrete case geven? Dan laat ik de engine dat doorrekenen en zien we waar de afwijking zit."* → gebruik Claude-orakel voor de trace.

**V**: *"Wet is niet mechanisch, je kan dit niet zomaar in formules gieten."*
**A**: *"Klopt. De formules dekken niet alle juridische nuance. Daarom noemen we een deel 'untranslatable' — bewust niet mechanisch. We willen van jullie horen waar die grens ligt."*

**V**: *"Waarom niet alles in de formules?"*
**A**: *"Wel dat wat mechanisch toetsbaar is. Alles wat discretie of oordeelsvorming vergt blijft in menselijke hand. Vandaag bepalen we samen welke zin waar valt."*

**V**: *"Maar deze wet kan morgen veranderen!"*
**A**: *"Daarom is de YAML gelabeld per datum (2026-02-07). Bij wetswijziging maken we een nieuwe YAML-versie — oude blijft beschikbaar voor aanslagen uit die periode."*
