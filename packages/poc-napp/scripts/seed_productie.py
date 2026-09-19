#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["requests"]
# ///
"""Vul een napp-omgeving met een realistische demowereld.

Draait volledig via de publieke API (geen directe databasetoegang) tegen elke
base-URL, lokaal of remote:

    packages/poc-napp/scripts/seed_productie.py       # http://localhost:8400
    packages/poc-napp/scripts/seed_productie.py https://poc.regelrecht.rijks.app/napp

De wereld: alle landelijke partijen (behalve de PVV) dienen hun jaaraanvraag
in met realistische ledentallen en neveninstellingen (seed_cast.json hiernaast);
~65% van de decentrale partijen en afdelingen doet hetzelfde (deterministische
steekproef uit packages/poc-napp/data/partijregister.json). Vrijwel alles wordt besloten
en bekendgemaakt, het merendeel van de voorschotten wordt uitbetaald, en een
handvol verhaal-dossiers (afwijzing, bezwaren, aangehouden betaling, open
claim, werkvoorraad) maakt elke tab van de beoordelaarsomgeving interessant.

Het script begint met POST /api/beheer/demo/reset en is daardoor idempotent:
opnieuw draaien geeft (op UUID's na) dezelfde wereld. De demo-voorbeelden uit
het loginscherm blijven onaangeroerd zodat bezoekers de flows zelf kunnen
doorlopen.
"""

from __future__ import annotations

import hashlib
import json
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import requests

REPO = Path(__file__).resolve().parent.parent
CAST = json.loads((REPO / "scripts" / "seed_cast.json").read_text())
REGISTER = json.loads((REPO / "data" / "partijregister.json").read_text())

BASE = sys.argv[1].rstrip("/") if len(sys.argv) > 1 else "http://localhost:8400"
WORKERS = 8
TIMEOUT = 120

SCHONE_VERKLARINGEN = {
    "ontvangt_anonieme_giften": False,
    "ontvangt_giften_niet_ingezetenen": False,
    "voldoet_aan_meldplicht_giften": True,
    "financien_openbaar_op_website": True,
}

BANKEN = ["ABNA", "INGB", "RABO", "TRIO", "SNSB", "ASNB", "KNAB", "BUNQ"]


def stable_fraction(key: str) -> float:
    """Deterministic pseudo-random in [0, 1) from a stable key."""
    digest = hashlib.md5(key.encode()).hexdigest()
    return int(digest[:8], 16) / 0x100000000


def demo_iban(kvk: str) -> str:
    """Deterministic, mod-97-valid NL IBAN for a KvK number."""
    bank = BANKEN[int(stable_fraction("bank:" + kvk) * len(BANKEN))]
    account = f"{int(stable_fraction('rek:' + kvk) * 10**10):010d}"
    digits = "".join(str(int(c, 36)) for c in bank + account + "NL00")
    check = 98 - int(digits) % 97
    return f"NL{check:02d}{bank}{account}"


class Api:
    """Thin client; one cookie-sessie per rol."""

    def __init__(self) -> None:
        self.http = requests.Session()

    def call(self, method: str, path: str, **kwargs):
        for poging in (1, 2, 3):
            try:
                r = self.http.request(method, BASE + path, timeout=TIMEOUT, **kwargs)
            except requests.ConnectionError:
                if poging == 3:
                    raise
                time.sleep(2 * poging)
                continue
            if r.status_code >= 500 and poging < 3:
                time.sleep(2 * poging)
                continue
            if r.status_code >= 400:
                raise RuntimeError(f"{method} {path} -> {r.status_code}: {r.text[:300]}")
            return r.json() if r.text else None
        raise RuntimeError(f"{method} {path}: geen geldige respons")

    def login_aanvrager(self, kvk: str) -> "Api":
        self.call("POST", "/api/eherkenning/login", json={"kvk_nummer": kvk})
        return self

    def login_beoordelaar(self, naam: str = "Seed (Napp-demo)") -> "Api":
        self.call("POST", "/api/sso/mock-login", json={"naam": naam})
        return self


_thread_local = threading.local()


def beoordelaar() -> Api:
    """Per-thread beoordelaarsessie (requests.Session is niet thread-safe)."""
    if not hasattr(_thread_local, "beoordelaar"):
        _thread_local.beoordelaar = Api().login_beoordelaar()
    return _thread_local.beoordelaar


# ---------------------------------------------------------------------------
# Castselectie
# ---------------------------------------------------------------------------

LANDELIJK = {p["kvk_nummer"]: p for p in CAST["landelijk"]}
VERHALEN = CAST["verhalen"]
SPEELTUIN = set(CAST["speeltuin"]["kvk_nummers"])
DIENT_NIET_IN = set(CAST["dient_niet_in"])
WERKVOORRAAD = set(VERHALEN["werkvoorraad_decentraal"]["kvk_nummers"])
# Geen rekening: het aangehouden-betaling-dossier (bewust) en de fictieve
# rechtspersoon (staat niet in het register; rekening opgeven kan dan niet).
ZONDER_REKENING = {
    VERHALEN["aangehouden_betaling"]["kvk_nummer"],
    VERHALEN["afwijzing_ledendrempel"]["kvk_nummer"],
}
VERHAAL_DECENTRAAL = (
    ZONDER_REKENING
    | {VERHALEN["bezwaar_open_staffel"]["kvk_nummer"]}
    | {VERHALEN["bezwaar_vormgebrek"]["kvk_nummer"]}
    | WERKVOORRAAD
)


def decentrale_cast() -> list[dict]:
    """Decentrale partijen/afdelingen die meedoen: de verhaal-dossiers plus
    een deterministische steekproef (~uptake) uit de rest."""
    uptake = CAST["decentrale_uptake"]
    cast = []
    for p in REGISTER["partijen"]:
        kvk = p["kvk_nummer"]
        if p["kamerzetels"] > 0 or p.get("status") == "ONGEKOPPELD":
            continue
        if kvk in SPEELTUIN or kvk in DIENT_NIET_IN or kvk in LANDELIJK:
            continue
        if not p["decentrale_uitslagen"]:
            continue
        if kvk in VERHAAL_DECENTRAAL or stable_fraction("uptake:" + kvk) < uptake:
            cast.append(p)
    return cast


# ---------------------------------------------------------------------------
# Fases
# ---------------------------------------------------------------------------


def dien_aanvraag_in(kvk: str, naam: str, parameters: dict) -> dict | None:
    """Login, rekening opgeven, jaaraanvraag indienen. Geeft het dossier
    terug, of None wanneer er niets aan te vragen valt."""
    api = Api().login_aanvrager(kvk)
    if kvk not in ZONDER_REKENING:
        api.call(
            "PUT",
            "/api/mijn-rekening",
            json={"iban": demo_iban(kvk), "tenaamstelling": naam},
        )
    registratie = api.call("GET", "/api/mijn-registratie")
    keys = [a["key"] for a in registratie["aanspraken"] if a["status"] == "BESCHIKBAAR"]
    if not keys:
        return None
    aanvraag = api.call(
        "POST",
        "/api/aanvragen",
        json={"componenten": keys, "parameters": SCHONE_VERKLARINGEN | parameters},
    )
    return {"kvk": kvk, "naam": naam, "aanvraag_id": aanvraag["id"]}


def landelijke_parameters(p: dict) -> dict:
    return {
        "aantal_betalende_leden": p["aantal_betalende_leden"],
        "heeft_wetenschappelijk_instituut": p["heeft_wetenschappelijk_instituut"],
        "heeft_jongerenorganisatie": p["heeft_jongerenorganisatie"],
        "aantal_leden_jongerenorganisatie": p["aantal_leden_jongerenorganisatie"],
        "heeft_instelling_buitenland": p["heeft_instelling_buitenland"],
    }


def beslis_en_maak_bekend(dossier: dict) -> dict:
    api = beoordelaar()
    besluit = api.call("POST", f"/api/aanvragen/{dossier['aanvraag_id']}/besluit")
    api.call("POST", f"/api/aanvragen/{dossier['aanvraag_id']}/bekendmaking")
    return dossier | {
        "besluit_id": besluit["besluit_id"],
        "toegekend": besluit["uitkomst"]["subsidie_toegekend"],
    }


def dien_bezwaar_in(kvk: str, besluit_id: str, bezwaar: dict) -> str:
    api = Api().login_aanvrager(kvk)
    body = {
        "naam_indiener": bezwaar["naam_indiener"],
        "adres_indiener": bezwaar.get("adres_indiener"),
        "gronden": bezwaar.get("gronden"),
        "ondertekend": bezwaar["ondertekend"],
    }
    return api.call("POST", f"/api/besluiten/{besluit_id}/bezwaar", json=body)["id"]


def beslis_bezwaar(bezwaar_id: str, bezwaar: dict) -> None:
    api = beoordelaar()
    api.call("POST", f"/api/bezwaren/{bezwaar_id}/horen", json={"gehoord": True})
    body: dict = {"beslissing": bezwaar["beslissing"]}
    if "gecorrigeerde_parameters" in bezwaar:
        body["gecorrigeerde_parameters"] = bezwaar["gecorrigeerde_parameters"]
    api.call("POST", f"/api/bezwaren/{bezwaar_id}/beslissen", json=body)


def main() -> None:
    print(f"Seed tegen {BASE}")
    print("— reset…")
    Api().login_beoordelaar().call("POST", "/api/beheer/demo/reset")

    decentraal = decentrale_cast()
    indieners: list[tuple[str, str, dict]] = []
    for p in CAST["landelijk"]:
        indieners.append((p["kvk_nummer"], p["naam"], landelijke_parameters(p)))
    for p in decentraal:
        indieners.append((p["kvk_nummer"], p["naam"], {}))
    drempel = VERHALEN["afwijzing_ledendrempel"]
    indieners.append(
        (
            drempel["kvk_nummer"],
            f"Organisatie {drempel['kvk_nummer']}",
            {"aantal_betalende_leden": drempel["aantal_betalende_leden"]},
        )
    )

    print(f"— {len(indieners)} aanvragers ({len(CAST['landelijk'])} landelijk, {len(decentraal)} decentraal)…")
    dossiers: list[dict] = []
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        for resultaat in pool.map(lambda i: dien_aanvraag_in(*i), indieners):
            if resultaat:
                dossiers.append(resultaat)
                if len(dossiers) % 100 == 0:
                    print(f"   {len(dossiers)} aanvragen ingediend")
    print(f"   {len(dossiers)} aanvragen ingediend")

    # Werkvoorraad blijft onbeslist; al het andere krijgt besluit + bekendmaking.
    # Pas ná alle aanvragen (de ledencomponent deelt het budget door het
    # totaal van alle opgaven van het subsidiejaar).
    blijft_open = WERKVOORRAAD | {
        p["kvk_nummer"] for p in CAST["landelijk"] if p.get("blijft_in_behandeling")
    }
    te_beslissen = [d for d in dossiers if d["kvk"] not in blijft_open]
    print(f"— {len(te_beslissen)} besluiten + bekendmakingen ({len(dossiers) - len(te_beslissen)} blijven in behandeling)…")
    besloten: list[dict] = []
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        for resultaat in pool.map(beslis_en_maak_bekend, te_beslissen):
            besloten.append(resultaat)
            if len(besloten) % 100 == 0:
                print(f"   {len(besloten)} besloten")
    toegekend = sum(1 for d in besloten if d["toegekend"])
    print(f"   {len(besloten)} besloten ({toegekend} toegekend, {len(besloten) - toegekend} afgewezen)")

    # Voorschotten: het merendeel van de klaargezette opdrachten wordt
    # uitbetaald; de rest blijft AANGEMAAKT (en de aangehouden opdracht van
    # het verhaal-dossier blijft AANGEHOUDEN).
    api = Api().login_beoordelaar()
    opdrachten = api.call("GET", "/api/betaalopdrachten")
    klaar = [o for o in opdrachten if o["status"] == "AANGEMAAKT"]
    te_betalen = [
        o for o in klaar
        if stable_fraction("uitbetaling:" + o["partij_naam"]) < CAST["uitbetaald_fractie"]
    ]
    print(f"— {len(te_betalen)} van {len(klaar)} voorschotten uitbetalen…")
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        list(pool.map(
            lambda o: beoordelaar().call("POST", f"/api/betaalopdrachten/{o['id']}/uitbetalen"),
            te_betalen,
        ))

    print("— verhaal-dossiers (bezwaren + claim)…")
    besluit_per_kvk = {d["kvk"]: d["besluit_id"] for d in besloten}

    # Afwijzing fictieve beweging: bezwaar wordt ongegrond verklaard.
    bezwaar_id = dien_bezwaar_in(
        drempel["kvk_nummer"], besluit_per_kvk[drempel["kvk_nummer"]], drempel["bezwaar"]
    )
    beslis_bezwaar(bezwaar_id, drempel["bezwaar"])

    # JA21 vergat de jongerenorganisatie: gegrond, herzien besluit (AWB 7:11).
    ja21 = VERHALEN["bezwaar_gegrond_jongerenorganisatie"]
    bezwaar_id = dien_bezwaar_in(ja21["kvk_nummer"], besluit_per_kvk[ja21["kvk_nummer"]], ja21["bezwaar"])
    beslis_bezwaar(bezwaar_id, ja21["bezwaar"])

    # Staffel-bezwaar: blijft in behandeling (werkvoorraad bezwaren).
    staffel = VERHALEN["bezwaar_open_staffel"]
    dien_bezwaar_in(staffel["kvk_nummer"], besluit_per_kvk[staffel["kvk_nummer"]], staffel["bezwaar"])

    # Niet-ondertekend bezwaarschrift: herstelfase (AWB 6:6).
    vormgebrek = VERHALEN["bezwaar_vormgebrek"]
    dien_bezwaar_in(vormgebrek["kvk_nummer"], besluit_per_kvk[vormgebrek["kvk_nummer"]], vormgebrek["bezwaar"])

    # Open claim op een ongekoppelde aanduiding.
    claim = VERHALEN["open_claim"]
    Api().login_aanvrager(claim["kvk_nummer"]).call(
        "POST", "/api/claim", json={"doel_kvk": claim["doel_kvk"]}
    )

    print("— controle: statistieken uit het openbaar register…")
    stats = Api().call("GET", "/api/register/statistieken")
    print(json.dumps(stats, indent=2, ensure_ascii=False))
    print("Klaar.")


if __name__ == "__main__":
    main()
