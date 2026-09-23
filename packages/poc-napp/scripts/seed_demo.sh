#!/usr/bin/env bash
# Vult een draaiende backend met demo-dossiers: drie partijen dienen hun
# jaaraanvraag in; twee worden besloten en bekendgemaakt, één blijft in
# behandeling. D66 en Hart voor Den Haag geven eerst hun rekening op
# (betaalopdracht met IBAN); de CDA-afdeling bewust niet, zodat een
# toekenning daar een AANGEHOUDEN betaalopdracht oplevert.
# Draaien: packages/poc-napp/scripts/seed_demo.sh [backend-url]
set -euo pipefail

B="${1:-http://localhost:8400}"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

zet_rekening() { # kvk iban tenaamstelling
  local jar="$TMP/$1-rekening.jar"
  curl -sf -c "$jar" -X POST "$B/api/eherkenning/login" \
    -H 'Content-Type: application/json' -d "{\"kvk_nummer\":\"$1\"}" > /dev/null
  curl -sf -b "$jar" -X PUT "$B/api/mijn-rekening" \
    -H 'Content-Type: application/json' \
    -d "{\"iban\":\"$2\",\"tenaamstelling\":\"$3\"}" > /dev/null
}

aanvraag() { # kvk leden -> aanvraag-id
  local kvk="$1" leden="$2" jar="$TMP/$1.jar"
  curl -sf -c "$jar" -X POST "$B/api/eherkenning/login" \
    -H 'Content-Type: application/json' -d "{\"kvk_nummer\":\"$kvk\"}" > /dev/null
  local keys
  keys=$(curl -sf -b "$jar" "$B/api/mijn-registratie" \
    | python3 -c "import sys,json;print(json.dumps([a['key'] for a in json.load(sys.stdin)['aanspraken'] if a['status']=='BESCHIKBAAR']))")
  python3 - "$keys" "$leden" <<'PY' | curl -sf -b "$jar" -X POST "$B/api/aanvragen" -H 'Content-Type: application/json' -d @- | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])"
import json, sys
print(json.dumps({
    "componenten": json.loads(sys.argv[1]),
    "parameters": {
        "aantal_betalende_leden": int(sys.argv[2]),
        "ontvangt_anonieme_giften": False,
        "ontvangt_giften_niet_ingezetenen": False,
        "voldoet_aan_meldplicht_giften": True,
        "financien_openbaar_op_website": True,
    },
}))
PY
}

# Beoordelaar-sessie (werkt alleen met mock-SSO, dus zonder OIDC-config).
JARB="$TMP/beoordelaar.jar"
curl -sf -c "$JARB" -X POST "$B/api/sso/mock-login" \
  -H 'Content-Type: application/json' -d '{"naam":"Demo Beoordelaar"}' > /dev/null

besluit_en_bekendmaking() { # aanvraag-id
  curl -sf -b "$JARB" -X POST "$B/api/aanvragen/$1/besluit" > /dev/null
  curl -sf -b "$JARB" -X POST "$B/api/aanvragen/$1/bekendmaking" > /dev/null
}

echo "D66 (landelijk + 240 gemeenten + 12 provincies)..."
zet_rekening 92525446 NL91ABNA0417164300 "D66"
ID_D66=$(aanvraag 92525446 24000)
besluit_en_bekendmaking "$ID_D66"

echo "Hart voor Den Haag / Groep de Mos (lokale partij)..."
zet_rekening 98626816 NL69INGB0123456789 "Hart voor Den Haag / Groep de Mos"
ID_HVDH=$(aanvraag 98626816 0)
besluit_en_bekendmaking "$ID_HVDH"

echo "CDA, afdeling 's-Gravenhage (afdeling met eigen rechtspersoon)..."
# Bewust géén rekening: een toekenning levert hier een AANGEHOUDEN
# betaalopdracht op (demomateriaal voor de rekening-flow).
aanvraag 99399789 0 > /dev/null   # blijft in behandeling: werkvoorraad-demo

echo "klaar: 2 bekendgemaakte besluiten (betaalopdrachten met IBAN), 1 aanvraag in behandeling (zonder rekening)"
