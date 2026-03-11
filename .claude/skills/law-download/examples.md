# Dutch Law Downloader - Usage Examples

## Example 1: Download Wet op de Zorgtoeslag

**User Request:**
```
Download the Wet op de Zorgtoeslag
```

**Skill Actions:**

1. **Search SRU API:**
   ```
   Query: dcterms.title any "zorgtoeslag"
   URL: http://zoekservice.overheid.nl/sru/Search?operation=searchRetrieve&version=1.2&x-connection=BWB&query=dcterms.title%20any%20%22zorgtoeslag%22
   ```

2. **Present Results:**
   ```
   Found 2 results:

   1. Wet op de zorgtoeslag
      BWBR ID: BWBR0018451
      Type: wet
      Latest version: 2025-01-01

   2. Besluit zorgtoeslag
      BWBR ID: BWBR0018452
      Type: AMvB
      Latest version: 2024-12-01

   Which law would you like to download? (Enter number or BWBR ID)
   ```

3. **User Selects:** "1"

4. **Download XML Files:**
   - WTI: `https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/BWBR0018451.WTI`
   - Toestand: `https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/2025-01-01/xml/BWBR0018451_2025-01-01.xml`

5. **Parse and Convert:**
   - Extract metadata from WTI
   - Extract 23 articles from Toestand
   - Convert XML formatting to markdown
   - Generate YAML

6. **Save File:**
   ```
   regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml
   ```

7. **Confirm:**
   ```
   ✓ Downloaded and converted Wet op de zorgtoeslag
     BWBR ID: BWBR0018451
     Articles: 23
     Saved to: regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml

   The YAML file contains the legal text only.
   To add machine-readable execution logic, use the law-machine-readable-interpreter skill.
   ```

---

## Example 2: Download by BWBR ID

**User Request:**
```
Get BWBR0018451 for date 2024-07-01
```

**Skill Actions:**

1. **Direct Query:**
   ```
   Query: dcterms.identifier==BWBR0018451
   ```

2. **Find Available Versions:**
   - Download manifest.xml
   - Check if 2024-07-01 exists
   - If exists, use it; otherwise suggest nearest date

3. **Download and Convert:**
   - Same process as Example 1
   - Use specified date: 2024-07-01

4. **Save File:**
   ```
   regulation/nl/wet/wet_op_de_zorgtoeslag/2024-07-01.yaml
   ```

---

## Example 3: Download Ministeriele Regeling

**User Request:**
```
Download the Regeling standaardpremie
```

**Skill Actions:**

1. **Search:**
   ```
   Query: dcterms.title any "standaardpremie"
   ```

2. **Results:**
   ```
   Found 1 result:

   1. Regeling standaardpremie zorgverzekeringswet
      BWBR ID: BWBR0020307
      Type: ministeriele regeling
      Latest version: 2025-01-01
   ```

3. **Download and Convert:**
   - Extract 4 articles
   - Map type to "MINISTERIELE_REGELING"

4. **Save File:**
   ```
   regulation/nl/ministeriele_regeling/regeling_standaardpremie_zorgverzekeringswet/2025-01-01.yaml
   ```

---

## Example 4: Handling Complex Formatting

**Input XML (Article 2 with formatting):**
```xml
<artikel eId="chp_1__art_2" wId="BWBR0018451_2025-01-01_0_art_2">
  <kop>
    <label>Artikel</label>
    <nr status="officieel">2</nr>
  </kop>
  <lid eId="chp_1__art_2__para_1" status="goed">
    <lidnr status="officieel">1</lidnr>
    <al>
      Een persoon heeft recht op zorgtoeslag indien hij:
    </al>
    <lijst level="single" nr-sluiting="." start="1" type="expliciet">
      <li>
        <li.nr>a.</li.nr>
        <al>de <nadruk type="cur">leeftijd van 18 jaar</nadruk> heeft bereikt;</al>
      </li>
      <li>
        <li.nr>b.</li.nr>
        <al>
          verzekerd is ingevolge de
          <extref doc="1.0:v:BWBR0018450" compleet="nee">Zorgverzekeringswet</extref>;
        </al>
      </li>
    </lijst>
  </lid>
  <lid eId="chp_1__art_2__para_2" status="goed">
    <lidnr status="officieel">2</lidnr>
    <al>
      Het <nadruk type="vet">toetsingsinkomen</nadruk> bedraagt niet meer dan
      <extref doc="1.0:v:BWBR0020307#art1" compleet="nee">€2.112</extref>.
    </al>
  </lid>
</artikel>
```

**Output YAML:**
```yaml
articles:
  - number: "2"
    text: |
      Een persoon heeft recht op zorgtoeslag indien hij:

      a. de *leeftijd van 18 jaar* heeft bereikt;
      b. verzekerd is ingevolge de [Zorgverzekeringswet](https://wetten.overheid.nl/BWBR0018450);

      Het **toetsingsinkomen** bedraagt niet meer dan [€2.112](https://wetten.overheid.nl/BWBR0020307#art1).
    url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2"
```

**Note:** Text is preserved exactly, including formatting and the original monetary notation (€2.112, not converted to eurocent).

---

## Example 5: Multiple Versions

**User Request:**
```
Download all versions of BWBR0018451 from 2020 onwards
```

**Skill Actions:**

1. **Download Manifest:**
   ```
   https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/manifest.xml
   ```

2. **Parse Available Dates:**
   ```
   Found 8 versions since 2020:
   - 2020-01-01
   - 2021-01-01
   - 2021-07-01
   - 2022-01-01
   - 2023-01-01
   - 2024-01-01
   - 2024-07-01
   - 2025-01-01
   ```

3. **Download Each Version:**
   - Loop through dates
   - Download and convert each
   - Save to separate YAML files

4. **Result:**
   ```
   regulation/nl/wet/wet_op_de_zorgtoeslag/
   ├── 2020-01-01.yaml
   ├── 2021-01-01.yaml
   ├── 2021-07-01.yaml
   ├── 2022-01-01.yaml
   ├── 2023-01-01.yaml
   ├── 2024-01-01.yaml
   ├── 2024-07-01.yaml
   └── 2025-01-01.yaml
   ```

---

## Example 6: Error Handling - Law Not Found

**User Request:**
```
Download the "Wet op het Groentetuintje"
```

**Skill Actions:**

1. **Search:**
   ```
   Query: dcterms.title any "groentetuintje"
   Results: 0
   ```

2. **Response:**
   ```
   No laws found matching "Wet op het Groentetuintje".

   Suggestions:
   - Check the exact title on wetten.overheid.nl
   - Try alternative search terms
   - Provide the BWBR ID if you know it

   Would you like to try a different search?
   ```

---

## Example 7: Full Output YAML

**Complete example of downloaded YAML file:**

```yaml
$schema: https://raw.githubusercontent.com/MinBZK/poc-machine-law/refs/heads/main/schema/v0.2.0/schema.json
$id: "wet_op_de_zorgtoeslag"
uuid: "a7b3c4d5-e6f7-8a9b-0c1d-2e3f4a5b6c7d"
regulatory_layer: "WET"
publication_date: "2005-12-30"
effective_date: "2006-01-01"

identifiers:
  bwb_id: "BWBR0018451"
  url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01"

articles:
  - number: "1"
    text: |
      In deze wet en de daarop berustende bepalingen wordt verstaan onder:

      a. Onze Minister: Onze Minister van Volksgezondheid, Welzijn en Sport;
      b. toeslagpartner: de persoon die als partner, bedoeld in [artikel 3 van de Algemene wet inkomensafhankelijke regelingen](https://wetten.overheid.nl/BWBR0018472#Hoofdstuk2), wordt aangemerkt;
      c. zorgtoeslag: de tegemoetkoming, bedoeld in artikel 2.
    url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1"

  - number: "2"
    text: |
      Een persoon heeft recht op zorgtoeslag indien hij:

      a. de *leeftijd van 18 jaar* heeft bereikt;
      b. verzekerd is ingevolge de [Zorgverzekeringswet](https://wetten.overheid.nl/BWBR0018450);
      c. in Nederland woont;
      d. rechtmatig in Nederland verblijft;
      e. beschikt over een **vermogen** dat niet meer bedraagt dan de in [artikel 3 van de Wet op de zorgtoeslag](https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel3) genoemde grens.

      Het toetsingsinkomen, bedoeld in [artikel 8 van de Algemene wet inkomensafhankelijke regelingen](https://wetten.overheid.nl/BWBR0018472#Artikel8), bedraagt niet meer dan de in [artikel 1 van de Regeling standaardpremie zorgverzekeringswet](https://wetten.overheid.nl/BWBR0020307#Artikel1) genoemde standaardpremie, vermenigvuldigd met de in dat artikel genoemde normpremiepercentages.
    url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2"

  - number: "3"
    text: |
      De in artikel 2, eerste lid, onder e, bedoelde grens bedraagt:

      a. € 154.859 voor een alleenstaande;
      b. € 186.875 voor gehuwden of samenwonenden.
    url: "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel3"

  # ... more articles ...
```

**Note:**
- No `machine_readable` sections
- Text preserved with markdown formatting
- Links converted to markdown format
- Monetary amounts kept as-is (€ notation)
- All articles included

---

## Integration with Machine-Readable Interpreter

After downloading with this skill, use the `law-machine-readable-interpreter` skill:

```
User: "Now interpret the zorgtoeslag law"

[law-machine-readable-interpreter activates]

Result: The same YAML file updated with machine_readable sections:
- Endpoints identified
- Parameters extracted
- Operations defined
- Cross-law references added with TODOs
- Monetary amounts converted to eurocent
```

This two-step workflow separates:
1. **Text acquisition** (downloader) - faithful to source
2. **Computational interpretation** (interpreter) - adding execution logic
