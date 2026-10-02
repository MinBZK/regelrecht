#!/usr/bin/env python3
"""Bouwt packages/poc-napp/data/partijregister.json uit open data.

Bronnen (alle Kiesraad-datasets via data.overheid.nl, CBS via StatLine):
- Verkiezingsuitslag Tweede Kamer 2025 (CSV): landelijke kamerzetels.
- Verkiezingsuitslagen Gemeenteraad 2026 (CSV): raadszetels per gemeente.
- Verkiezingsuitslagen Provinciale Staten 2023 (EML, Resultaat-bestanden):
  statenzetels per provincie (gekozen kandidaten per lijst).
- Verkiezingsuitslagen Waterschappen 2023 (EML, Resultaat-bestanden):
  AB-zetels per waterschap.
- Verkiezingsuitslag Eilandsraad 2023 (CSV): eilandsraadszetels per
  openbaar lichaam (Bonaire, Sint Eustatius, Saba). De Kiesraad publiceert
  deze uitslag alleen als CSV, niet als EML.
- CBS StatLine 37230ned (OData): inwoneraantal per gemeente en provincie
  (januari 2026).
- CBS StatLine 83698NED (OData): inwoneraantal Caribisch Nederland per
  openbaar lichaam (1 januari 2026); de openbare lichamen staan niet in
  37230ned.

KvK-nummers zijn synthetisch en deterministisch: de koppeling
rechtspersoon-aanduiding is geen open data en is precies wat de Napp bij
registratie vastlegt.

Organisatiemodellen (Wpp, MvT bij art. 27): bij CENTRAAL georganiseerde
partijen zijn afdelingen geen rechtspersoon en valt alles onder de
landelijke vereniging (één KvK). Bij DECENTRAAL georganiseerde partijen is
elke afdeling een eigen vereniging met eigen KvK. Welke partijen decentraal
georganiseerd zijn volgt uit hun statuten en is geen open data; de set
hieronder is een demo-aanname.

Naamaliassen: lijstnamen verschillen per verkiezing (GROENLINKS en PvdA
deden in 2023 apart mee, in TK2025 als gezamenlijke lijst). De alias-tabel
koppelt varianten aan de TK2025-lijstnaam.

Draaien: uv run scripts/bouw_register.py
"""

import csv
import hashlib
import io
import json
import urllib.request
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

TK2025_CSV_ZIP = (
    "https://data.overheid.nl/sites/default/files/dataset/"
    "a16f3352-c9ce-4831-a314-f989d442a258/resources/"
    "Verkiezingsuitslag%20Tweede%20Kamer%202025%20%28CSV%20Formaat%29.zip"
)
GR2026_CSV = (
    "https://data.overheid.nl/sites/default/files/dataset/"
    "09cf04e7-11ac-4b53-a2cc-baccf95e82fd/resources/GR2026.csv"
)
PS2023_EML_ZIPS = [
    "https://data.overheid.nl/sites/default/files/dataset/"
    f"be8b7869-4a12-4446-abab-5cd0a436dc4f/resources/EML_bestanden_PS2023_deel_{i}.zip"
    for i in (1, 2, 3)
]
AB2023_EML_ZIPS = [
    "https://data.overheid.nl/sites/default/files/dataset/"
    f"ee19bcda-0282-44a6-a464-44738afd755c/resources/EML_bestanden_AB2023_deel_{i}.zip"
    for i in (1, 2, 3)
]
ER2023_CSV = (
    "https://data.overheid.nl/sites/default/files/dataset/"
    "139bd69c-dfc7-47e3-9cfa-79d789769079/resources/Verkiezingsuitslag_ER2023.csv"
)
CBS_BEVOLKING = (
    "https://opendata.cbs.nl/ODataApi/odata/37230ned/TypedDataSet"
    "?$filter=(startswith(RegioS,'GM')%20or%20startswith(RegioS,'PV'))"
    "%20and%20Perioden%20eq%20'2026MM01'"
    "&$select=RegioS,BevolkingAanHetBeginVanDePeriode_1&$top=10000&$format=json"
)
# Bevolking Caribisch Nederland: eigen StatLine-tabel (83698NED); totalen
# (geslacht, leeftijd, burgerlijke staat) per openbaar lichaam, 1 jan 2026.
CBS_BEVOLKING_CN = (
    "https://opendata.cbs.nl/ODataApi/odata/83698NED/TypedDataSet"
    "?$filter=startswith(CaribischNederland,'GM')"
    "%20and%20Geslacht%20eq%20'T001038'%20and%20Leeftijd%20eq%20'10000'"
    "%20and%20BurgerlijkeStaat%20eq%20'T001019'"
    "%20and%20Perioden%20eq%20'2026JJ00'"
    "&$select=CaribischNederland,BevolkingOp1Januari_1&$format=json"
)

# CBS-codes van de twaalf provincies.
PROVINCIE_CODES = {
    "Groningen": "PV20", "Fryslan": "PV21", "Fryslân": "PV21", "Drenthe": "PV22",
    "Overijssel": "PV23", "Flevoland": "PV24", "Gelderland": "PV25",
    "Utrecht": "PV26", "Noord-Holland": "PV27", "Zuid-Holland": "PV28",
    "Zeeland": "PV29", "Noord-Brabant": "PV30", "Limburg": "PV31",
}

# Demo-aanname: decentraal georganiseerde partijen (afdelingen met eigen
# rechtspersoon en eigen KvK). Niet uit open data af te leiden.
DECENTRAAL_GEORGANISEERD = {
    "CDA",
    "ChristenUnie",
    "Staatkundig Gereformeerde Partij (SGP)",
}

# Lijstnaam-varianten → TK2025-lijstnaam.
ALIASSEN = {
    "GROENLINKS": "GROENLINKS / Partij van de Arbeid (PvdA)",
    "Partij van de Arbeid (P.v.d.A.)": "GROENLINKS / Partij van de Arbeid (PvdA)",
}

# Demo-aanname voor de claim-flow: drie aanduidingen uit de uitslag waarvan
# de rechtspersoon nog niet bekend is bij de Napp (status ONGEKOPPELD). De
# partij claimt haar aanduiding bij de eerste aanvraag; tot die tijd is het
# KvK-nummer in het register een placeholder. Keuze: drie herkenbare lokale
# partijen zonder bestaande demo-rol, uit drie verschillende gemeenten, elk
# ruim boven de twee raadszetels (GR2026):
# - Leefbaar Capelle (Capelle aan den IJssel, 15 zetels)
# - Wakker Emmen (Emmen, 14 zetels)
# - EVB (Echt voor Barendrecht) (Barendrecht, 14 zetels)
ONGEKOPPELDE_AANDUIDINGEN = {
    "Leefbaar Capelle",
    "Wakker Emmen",
    "EVB (Echt voor Barendrecht)",
}

CACHE = Path("/tmp/napp_bronnen")
UIT = Path(__file__).resolve().parent.parent / "data" / "partijregister.json"


def haal(url: str) -> bytes:
    CACHE.mkdir(exist_ok=True)
    naam = CACHE / hashlib.sha1(url.encode()).hexdigest()
    if naam.exists():
        return naam.read_bytes()
    print(f"downloaden: {url[:90]}...")
    with urllib.request.urlopen(url) as response:
        data = response.read()
    naam.write_bytes(data)
    return data


def kvk_voor(sleutel: str) -> str:
    """Deterministisch synthetisch KvK-nummer (8 cijfers, begint met 9)."""
    digest = hashlib.sha1(sleutel.encode()).hexdigest()
    return "9" + str(int(digest, 16) % 10_000_000).zfill(7)


def gebiedsnaam_uit_resultaat_eml(xml_bytes: bytes) -> str | None:
    """De officiële gebiedsnaam uit een Resultaat-EML (kr:ElectionDomain),
    bv. 'Aa en Maas' of 'Noord-Brabant' — netter dan de mapnaam in de zip."""
    root = ET.fromstring(xml_bytes)
    for e in root.iter():
        if e.tag.split("}")[-1] == "ElectionDomain":
            return (e.text or "").strip() or None
    return None


def zetels_uit_resultaat_eml(xml_bytes: bytes) -> dict[str, int]:
    """Zetels per lijst uit een Kiesraad Resultaat-EML: tel per lijst de
    gekozen kandidaten (Elected = yes)."""
    root = ET.fromstring(xml_bytes)

    def tag(e):
        return e.tag.split("}")[-1]

    zetels: dict[str, int] = {}
    huidige = None
    for sel in root.iter():
        if tag(sel) != "Selection":
            continue
        naam, gekozen, is_kandidaat = None, False, False
        for k in sel.iter():
            t = tag(k)
            if t == "RegisteredName":
                naam = (k.text or "").strip()
            if t == "Candidate":
                is_kandidaat = True
            if t == "Elected" and (k.text or "").strip() == "yes":
                gekozen = True
        if naam is not None and not is_kandidaat:
            huidige = naam
            zetels.setdefault(huidige, 0)
        elif is_kandidaat and gekozen and huidige:
            zetels[huidige] += 1
    return zetels


def resultaten_uit_eml_zips(urls: list[str]) -> dict[str, tuple[str, dict[str, int]]]:
    """Map mapnaam → (officiële gebiedsnaam, lijstnaam → zetels) uit
    Resultaat-EML's in zips. De mapnaam is de stabiele sleutel voor de
    gebied-code; de officiële naam (kr:ElectionDomain) is voor weergave."""
    resultaten: dict[str, tuple[str, dict[str, int]]] = {}
    for url in urls:
        zf = zipfile.ZipFile(io.BytesIO(haal(url)))
        for naam in zf.namelist():
            basis = naam.rsplit("/", 1)[-1]
            if not (basis.startswith("Resultaat_") and basis.endswith(".eml.xml")):
                continue
            sleutel = naam.split("/")[1] if "/" in naam else basis
            xml = zf.read(naam)
            zetels = zetels_uit_resultaat_eml(xml)
            if zetels:
                officieel = gebiedsnaam_uit_resultaat_eml(xml) or sleutel
                resultaten[sleutel] = (officieel, zetels)
    return resultaten


def main() -> None:
    # --- TK2025: landelijke zetels ---
    zipdata = zipfile.ZipFile(io.BytesIO(haal(TK2025_CSV_ZIP)))
    naam_uitslag = next(n for n in zipdata.namelist() if n.endswith("TK2025_uitslag.csv"))
    landelijk: dict[str, int] = {}
    with zipdata.open(naam_uitslag) as f:
        tekst = io.TextIOWrapper(f, encoding="utf-8-sig")
        for rij in csv.DictReader(tekst, delimiter=";"):
            if rij["Regio"] == "Nederland" and rij["VeldType"] == "LijstAantalZetels":
                landelijk[rij["LijstNaam"]] = int(rij["Waarde"])
    print(f"TK2025: {len(landelijk)} landelijke partijen met zetels")

    # --- GR2026: raadszetels per gemeente ---
    gebied_namen: dict[str, str] = {}
    uitslagen: list[tuple[str, str, str, int]] = []  # (orgaan, gebiedcode, lijst, zetels)
    tekst = io.StringIO(haal(GR2026_CSV).decode("utf-8-sig"))
    for rij in csv.DictReader(tekst, delimiter=";"):
        if rij["VeldType"] != "LijstAantalZetels":
            continue
        code = rij["RegioCode"]
        if not code.startswith("G"):
            continue
        gm = "GM" + code[1:]
        gebied_namen[gm] = rij["Regio"]
        uitslagen.append(("GEMEENTERAAD", gm, rij["LijstNaam"], int(rij["Waarde"])))
    print(f"GR2026: {len(uitslagen)} uitslagen, {len(gebied_namen)} gemeenten")

    # --- PS2023: statenzetels per provincie (EML) ---
    ps = resultaten_uit_eml_zips(PS2023_EML_ZIPS)
    for provincie, (officieel, lijsten) in ps.items():
        code = PROVINCIE_CODES.get(provincie, f"PV_{provincie}")
        gebied_namen[code] = officieel
        for lijst, zetels in lijsten.items():
            uitslagen.append(("PROVINCIALE_STATEN", code, lijst, zetels))
    print(f"PS2023: {len(ps)} provincies")

    # --- AB2023: waterschapszetels (EML) ---
    ab = resultaten_uit_eml_zips(AB2023_EML_ZIPS)
    for waterschap, (officieel, lijsten) in ab.items():
        code = f"WS_{waterschap}"
        gebied_namen[code] = officieel
        for lijst, zetels in lijsten.items():
            uitslagen.append(("WATERSCHAP", code, lijst, zetels))
    print(f"AB2023: {len(ab)} waterschappen")

    # --- ER2023: eilandsraadszetels per openbaar lichaam (CSV) ---
    # De Kiesraad-CSV codeert de openbare lichamen als O9001 (Bonaire),
    # O9002 (Sint Eustatius) en O9003 (Saba); CBS gebruikt in de Caribisch
    # Nederland-tabellen GM9001/GM9002/GM9003. We herschrijven naar de
    # CBS-codes zodat de inwoneraantallen direct aansluiten (de openbare
    # lichamen hebben geen reguliere gemeentecode in 37230ned).
    er_telling = 0
    tekst = io.StringIO(haal(ER2023_CSV).decode("utf-8-sig"))
    for rij in csv.DictReader(tekst, delimiter=";"):
        if rij["VeldType"] != "LijstAantalZetels":
            continue
        code = rij["RegioCode"]
        if not code.startswith("O"):
            continue
        gm = "GM" + code[1:]
        gebied_namen[gm] = rij["Regio"]
        uitslagen.append(("EILANDSRAAD", gm, rij["LijstNaam"], int(rij["Waarde"])))
        er_telling += 1
    print(f"ER2023: {er_telling} uitslagen, 3 openbare lichamen")

    # --- CBS: inwoneraantallen (gemeenten + provincies) ---
    cbs = json.loads(haal(CBS_BEVOLKING))
    inwoners = {
        rij["RegioS"].strip(): rij["BevolkingAanHetBeginVanDePeriode_1"]
        for rij in cbs["value"]
        if rij["BevolkingAanHetBeginVanDePeriode_1"] is not None
    }
    cbs_cn = json.loads(haal(CBS_BEVOLKING_CN))
    for rij in cbs_cn["value"]:
        if rij["BevolkingOp1Januari_1"] is not None:
            inwoners[rij["CaribischNederland"].strip()] = rij["BevolkingOp1Januari_1"]
    print(f"CBS: inwoneraantallen voor {len(inwoners)} regio's")

    # --- Partijen samenstellen ---
    partijen: dict[str, dict] = {}
    for naam, zetels in sorted(landelijk.items()):
        kvk = kvk_voor(f"landelijk|{naam}")
        partijen[kvk] = {
            "kvk_nummer": kvk,
            "naam": naam,
            "organisatiemodel": "DECENTRAAL" if naam in DECENTRAAL_GEORGANISEERD else "CENTRAAL",
            "kamerzetels": zetels,
            "moederpartij_kvk": None,
            "decentrale_uitslagen": [],
        }

    for orgaan, gebied_code, lijstnaam, zetels in uitslagen:
        if zetels == 0:
            continue
        lijstnaam = ALIASSEN.get(lijstnaam, lijstnaam)
        uitslag = {"orgaan": orgaan, "gebied_code": gebied_code, "zetels": zetels}

        if lijstnaam in landelijk:
            if lijstnaam in DECENTRAAL_GEORGANISEERD:
                # Decentraal georganiseerd: de afdeling is een eigen
                # rechtspersoon met eigen KvK (Wpp-model 2).
                kvk = kvk_voor(f"afdeling|{lijstnaam}|{gebied_code}")
                if kvk not in partijen:
                    partijen[kvk] = {
                        "kvk_nummer": kvk,
                        "naam": f"{lijstnaam}, afdeling {gebied_namen[gebied_code]}",
                        "organisatiemodel": "DECENTRAAL",
                        "kamerzetels": 0,
                        "moederpartij_kvk": kvk_voor(f"landelijk|{lijstnaam}"),
                        "decentrale_uitslagen": [],
                    }
                partijen[kvk]["decentrale_uitslagen"].append(uitslag)
            else:
                # Centraal georganiseerd: alles onder de landelijke KvK.
                partijen[kvk_voor(f"landelijk|{lijstnaam}")]["decentrale_uitslagen"].append(uitslag)
        else:
            # Lokale/provinciale/waterschapspartij: eigen rechtspersoon.
            kvk = kvk_voor(f"lokaal|{gebied_code}|{lijstnaam}")
            if kvk not in partijen:
                partijen[kvk] = {
                    "kvk_nummer": kvk,
                    "naam": lijstnaam,
                    "organisatiemodel": "CENTRAAL",
                    "kamerzetels": 0,
                    "moederpartij_kvk": None,
                    "decentrale_uitslagen": [],
                }
                if lijstnaam in ONGEKOPPELDE_AANDUIDINGEN:
                    # Rechtspersoon nog onbekend: het KvK-nummer is een
                    # placeholder tot de partij haar aanduiding claimt.
                    partijen[kvk]["status"] = "ONGEKOPPELD"
            partijen[kvk]["decentrale_uitslagen"].append(uitslag)

    orgaan_van_code: dict[str, str] = {}
    for orgaan, code, _, _ in uitslagen:
        orgaan_van_code[code] = orgaan
    gebieden = [
        {
            "orgaan": orgaan_van_code.get(code, "GEMEENTERAAD"),
            "code": code,
            "naam": naam,
            "inwoneraantal": inwoners.get(code, 0),
        }
        for code, naam in sorted(gebied_namen.items(), key=lambda x: x[1])
    ]

    # --- Demo-voorbeelden voor de mock-login: een gevarieerd palet ---
    def voorbeeld(p, profiel):
        return {"kvk_nummer": p["kvk_nummer"], "naam": p["naam"], "profiel": profiel}

    # ONGEKOPPELDE aanduidingen kunnen geen demo-login zijn: hun KvK-nummer
    # is een placeholder; de claim-flow heeft een eigen demo-voorbeeld.
    alle = [p for p in partijen.values() if p.get("status") != "ONGEKOPPELD"]
    landelijke = [p for p in alle if p["kamerzetels"] > 0]
    lokale_gr = [
        p for p in alle
        if p["kamerzetels"] == 0 and p["moederpartij_kvk"] is None
        and p["decentrale_uitslagen"]
        and all(u["orgaan"] == "GEMEENTERAAD" for u in p["decentrale_uitslagen"])
    ]
    waterschaps = [
        p for p in alle
        if p["kamerzetels"] == 0 and p["moederpartij_kvk"] is None
        and p["decentrale_uitslagen"]
        and all(u["orgaan"] == "WATERSCHAP" for u in p["decentrale_uitslagen"])
    ]
    eiland = [
        p for p in alle
        if p["kamerzetels"] == 0 and p["moederpartij_kvk"] is None
        and p["decentrale_uitslagen"]
        and all(u["orgaan"] == "EILANDSRAAD" for u in p["decentrale_uitslagen"])
    ]

    grootste = max(landelijke, key=lambda p: (p["kamerzetels"], p["naam"]))
    breedste = max(landelijke, key=lambda p: len(p["decentrale_uitslagen"]))
    kleinste = min(landelijke, key=lambda p: (p["kamerzetels"], p["naam"]))
    meeste_ps = max(
        landelijke,
        key=lambda p: sum(u["zetels"] for u in p["decentrale_uitslagen"]
                          if u["orgaan"] == "PROVINCIALE_STATEN"),
    )
    afdeling = min(
        (p for p in alle if p["moederpartij_kvk"] is not None),
        key=lambda p: p["naam"],
    )
    grootste_lokaal = max(lokale_gr, key=lambda p: max(u["zetels"] for u in p["decentrale_uitslagen"]))
    kleinste_lokaal = min(lokale_gr, key=lambda p: (max(u["zetels"] for u in p["decentrale_uitslagen"]), p["naam"]))
    waterschap = max(waterschaps, key=lambda p: max(u["zetels"] for u in p["decentrale_uitslagen"]))
    # Grootste eilandspartij: DEMOKRAT en UPB hebben beide 3 zetels op
    # Bonaire; de alfabetische tiebreak kiest DEMOKRAT, dat ook de meeste
    # stemmen haalde (4.004 tegen 2.903, officiële uitslag ER2023).
    grootste_eiland = min(
        eiland,
        key=lambda p: (-max(u["zetels"] for u in p["decentrale_uitslagen"]), p["naam"]),
    )

    # Demo voor het machtigingenmodel: D66 is centraal georganiseerd, dus de
    # login biedt naast het tekenbevoegd bestuur ook afdelingsvolmachten aan.
    centraal_demo = next(
        (p for p in landelijke
         if p["organisatiemodel"] == "CENTRAAL" and p["naam"] == "D66"),
        None,
    )

    demo = [
        voorbeeld(grootste, f"grootste landelijke partij ({grootste['kamerzetels']} kamerzetels)"),
        voorbeeld(breedste, f"breedste decentrale dekking ({len(breedste['decentrale_uitslagen'])} gebieden)"),
        *([voorbeeld(centraal_demo, "centraal organisatiemodel: log in als bestuur of als afdeling (beperkte machtiging)")]
          if centraal_demo else []),
        voorbeeld(kleinste, f"kleinste landelijke partij ({kleinste['kamerzetels']} kamerzetel{'s' if kleinste['kamerzetels'] != 1 else ''}, op de ledendrempel)"),
        voorbeeld(meeste_ps, "sterk in provinciale staten"),
        voorbeeld(afdeling, "afdeling met eigen rechtspersoon (decentraal organisatiemodel)"),
        voorbeeld(grootste_lokaal, "grootste lokale partij"),
        voorbeeld(kleinste_lokaal, "lokale partij met een raadszetel"),
        voorbeeld(waterschap, "waterschapspartij"),
        voorbeeld(grootste_eiland, "grootste eilandspartij (eilandsraad Bonaire, Caribisch Nederland)"),
        # Claim-flow-demo: dit KvK-nummer staat bewust NIET in het register.
        # De rechtspersoon logt in, claimt een ongekoppelde aanduiding uit de
        # uitslag en wacht op bevestiging door een Napp-beoordelaar.
        {
            "kvk_nummer": "90000001",
            "naam": "Nieuwe rechtspersoon zonder koppeling",
            "profiel": "demonstreert de claim-flow: koppel deze rechtspersoon aan een ongekoppelde aanduiding uit de verkiezingsuitslag",
        },
    ]
    # dedupliceer (criteria kunnen samenvallen) met behoud van volgorde
    gezien = set()
    demo = [d for d in demo if not (d["kvk_nummer"] in gezien or gezien.add(d["kvk_nummer"]))]

    register = {
        "bronnen": {
            "landelijk": "Verkiezingsuitslag Tweede Kamer 2025 (Kiesraad, data.overheid.nl)",
            "gemeenteraden": "Verkiezingsuitslagen Gemeenteraad 2026 (Kiesraad, data.overheid.nl)",
            "provinciale_staten": "Verkiezingsuitslagen Provinciale Staten 2023, Resultaat-EML (Kiesraad)",
            "waterschappen": "Verkiezingsuitslagen Waterschappen 2023, Resultaat-EML (Kiesraad)",
            "eilandsraden": "Verkiezingsuitslag Eilandsraad 2023 (Kiesraad, data.overheid.nl, CSV)",
            "inwoneraantallen": "CBS StatLine 37230ned, januari 2026",
            "inwoneraantallen_caribisch_nederland": "CBS StatLine 83698NED, 1 januari 2026",
            "kvk_nummers": "synthetisch (koppeling rechtspersoon-aanduiding is geen open data)",
            "organisatiemodellen": "demo-aanname (volgt uit partijstatuten, geen open data)",
        },
        "partijen": sorted(partijen.values(), key=lambda p: (-p["kamerzetels"], p["naam"])),
        "gebieden": gebieden,
        "demo_voorbeelden": demo,
    }

    UIT.parent.mkdir(parents=True, exist_ok=True)
    UIT.write_text(json.dumps(register, ensure_ascii=False, indent=1))
    print(f"geschreven: {UIT} ({UIT.stat().st_size // 1024} kB, "
          f"{len(partijen)} partijen, {len(gebieden)} gebieden)")
    for d in demo:
        print(f"  demo-login: {d['kvk_nummer']} → {d['naam']} ({d['profiel']})")


if __name__ == "__main__":
    main()
