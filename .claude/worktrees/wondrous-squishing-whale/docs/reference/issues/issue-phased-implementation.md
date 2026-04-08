# Issue #2: Phased Implementation (Gefaseerde Inwerkingtreding)

## Problem Summary

Articles with phased implementation ("gefaseerde inwerkingtreding") contain **multiple versions of the same article content** within a single XML `<artikel>` element. The harvester currently splits by `<lid>` (paragraph), producing duplicate article numbers like `8:36c.1` appearing twice.

## Affected Laws

- **Awb (BWBR0005537)**: Artikel 8:36c - two versions of leden 1-4
- **Mediawet 2008 (BWBR0025028)**: Artikel 2.88.5 - two different texts

## Legal Background

### What is Gefaseerde Inwerkingtreding?

The KEI program (Kwaliteit en Innovatie rechtspraak / Quality and Innovation in Justice) digitizes civil and administrative court procedures. The enabling law (Stb. 2016, 288) allows different articles to enter into force at different times for different courts.

**Staatsblad 2016, 288 - ARTIKEL V, lid 1:**
> "De artikelen van deze wet treden in werking op een bij koninklijk besluit te bepalen tijdstip, dat voor de verschillende artikelen of onderdelen daarvan, voor verschillende vorderingen, verzoeken en besluiten en voor de verschillende gerechten en verschillende bestuursrechters verschillend kan worden vastgesteld."

Translation: Articles of this law enter into force at a time determined by royal decree, which may be set differently for different articles, claims, requests, decisions, courts, and administrative judges.

### Timeline for Article 8:36c

| Date | Publication | Effect |
|------|-------------|--------|
| 2016-07-21 | [Stb. 2016, 288](https://zoek.officielebekendmakingen.nl/stb-2016-288.html) | Original law passed |
| 2017-06-12 | [Stb. 2017, 174](https://zoek.officielebekendmakingen.nl/stb-2017-174.html) | First courts go live |
| 2020-04-15 | [Stb. 2020, 99](https://zoek.officielebekendmakingen.nl/stb-2020-99.html) | Tax cassation at Supreme Court |

As of 2024, **both versions remain valid simultaneously**:
- **Version 1**: Courts where digital proceedings are mandatory
- **Version 2**: "For other cases" - courts where KEI is not yet implemented

## XML Structure

See: `doc/issues/awb-2024-01-01.xml` (lines 6685-6731)

```xml
<artikel label="Artikel 8:36c" inwerking="2020-04-15" bron="Stb.2016-288">
  <kop>
    <label>Artikel</label>
    <nr status="officieel">8:36c</nr>
  </kop>

  <!-- EDITORIAL NOTE (not law text!) -->
  <al>
    <redactie type="extra">Dit artikel is gewijzigd in verband met de invoering
    van digitaal procederen. Zie voor de procedures en gerechten waarvoor digitaal
    procederen geldt het Overzicht gefaseerde inwerkingtreding op
    www.rijksoverheid.nl/KEI.</redactie>
  </al>

  <!-- VERSION 1: For courts with digital proceedings -->
  <lid bwb-ng-variabel-deel="...Lid1_1">
    <lidnr>1</lidnr>
    <al>Als tijdstip waarop een bericht door de bestuursrechter langs elektronische
    weg is ontvangen, geldt het tijdstip waarop het bericht het digitale systeem
    voor gegevensverwerking van de bestuursrechter heeft bereikt. Na elke indiening
    langs elektronische weg ontvangt de indiener een ontvangstbevestiging in het
    digitale systeem voor gegevensverwerking.</al>
    <meta-data>...</meta-data>
  </lid>
  <!-- lid 2, 3, 4 similar -->

  <!-- SEPARATOR: Editorial note + repeated header -->
  <al>
    <redactie type="extra">Voor overige gevallen luidt het artikel als volgt:</redactie>
  </al>
  <tussenkop kopopmaak="cur">Artikel 8:36c.</tussenkop>

  <!-- VERSION 2: For other courts (no digital proceedings yet) -->
  <lid bwb-ng-variabel-deel="...Lid1_2">
    <lidnr>1</lidnr>
    <al>Als tijdstip waarop een bericht door de bestuursrechter langs elektronische
    weg is ontvangen, geldt het tijdstip waarop het bericht het digitale systeem
    voor gegevensverwerking van de bestuursrechter heeft bereikt.</al>
  </lid>
  <!-- lid 2, 3, 4 similar - note: no receipt confirmation in version 2 -->

  <!-- Article-level metadata -->
  <meta-data>
    <brondata>
      <opmerkingen-inhoud>
        <al>Treedt in werking voor het beroep in cassatie bij de Hoge Raad
        waarop afdeling 4 van hoofdstuk V van de Algemene wet inzake
        rijksbelastingen van toepassing of van overeenkomstige toepassing is.</al>
      </opmerkingen-inhoud>
    </brondata>
  </meta-data>
</artikel>
```

## Key Observations

### 1. Version Markers

The two versions are separated by:
- `<al><redactie type="extra">Voor overige gevallen luidt het artikel als volgt:</redactie></al>`
- `<tussenkop>Artikel 8:36c.</tussenkop>` (repeats article header)

### 2. Editorial Notes (`<redactie>`)

**CRITICAL**: Elements like `<redactie type="extra">` are **editorial annotations**, NOT law text. They should be:
- Excluded from harvested article text
- Used only as structural markers

Examples of editorial content:
- "Dit artikel is gewijzigd in verband met de invoering van digitaal procederen..."
- "Voor overige gevallen luidt het artikel als volgt:"
- "Vervallen." (for repealed articles)
- "Dit onderdeel is nog niet in werking getreden"

### 3. Structural Differences

| Aspect | Version 1 | Version 2 |
|--------|-----------|-----------|
| `bwb-ng-variabel-deel` | `Lid1_1`, `Lid2_1`, etc. | `Lid1_2`, `Lid2_2`, etc. |
| `<meta-data>` inside lid | Present | Absent |
| Content | Includes receipt confirmation | No receipt confirmation |

### 4. Both Versions Are Legally Valid

This is NOT a case of old vs. new text. Both versions are **simultaneously valid** for different contexts. The consolidated XML represents the complete current state of the law.

## Current Harvester Behavior

The harvester:
1. Finds all `<lid>` elements as structural children
2. Creates a separate `ArticleComponent` for each
3. Numbers them: `8:36c.1`, `8:36c.2`, `8:36c.3`, `8:36c.4`
4. Then encounters version 2 leden and creates duplicates

Result: 8 components with duplicate numbers (8:36c.1 appears twice, etc.)

## Proposed Solution

### Approach: Keep Multi-Version Articles as Single Component

Articles containing `<tussenkop>` elements (which repeat the article header for alternative versions) should NOT be split by lid. Instead, extract the entire article as one component containing all versions.

### Additional Requirement: Exclude Editorial Content

All `<redactie>` elements must be excluded from the extracted text. They are editorial annotations, not law text.

## References

- [Staatsblad 2016, 288](https://zoek.officielebekendmakingen.nl/stb-2016-288.html) - Original KEI law
- [Staatsblad 2017, 174](https://zoek.officielebekendmakingen.nl/stb-2017-174.html) - First implementation decree
- [Staatsblad 2020, 99](https://zoek.officielebekendmakingen.nl/stb-2020-99.html) - Tax cassation implementation
- [KEI Overview](https://www.rijksoverheid.nl/onderwerpen/rechtspraak-en-geschiloplossing/kwaliteit-en-innovatie-kei-rechtspraak) - Government info page

## Test Data

- XML file: `doc/issues/awb-2024-01-01.xml`
- Article 8:36c location: lines 6685-6731
