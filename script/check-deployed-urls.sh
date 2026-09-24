#!/usr/bin/env bash
# Controleert of de zojuist uitgerolde componenten antwoorden.
#
# Na een deploy kijkt niets of productie leeft. De poort ervoor bewaakt dat er
# niets van een rood commit uitgaat en dat er geen halve uitrol plaatsvindt;
# geen van beide zegt iets over de uitkomst.
#
# De lijst URL's komt uit de deploy-action zelf (`urls`-output), niet uit een
# opsomming hier. Een hostnamenlijst in dit bestand zou wegdrijven zodra er een
# component bijkomt, en hij zou componenten controleren die deze run niet heeft
# aangeraakt.
set -uo pipefail

: "${URLS:?URLS is verplicht (JSON-object component -> url)}"

# Componenten die op `/` niets serveren, met het pad dat wél bewijst dat ze
# leven. De rest wordt op zijn hoofdadres bevraagd.
#
# harvester-admin is sinds #902 een kale API: zijn dashboard verhuisde naar de
# editor (frontend/src/harvester), en packages/admin/src/main.rs heeft bewust
# geen fallback meer, dus `/` geeft 404. Dat is het juiste antwoord van die
# dienst, geen storing; `/health` is de route die er wel is.
#
# Dit is een uitzonderingslijst, geen vrijbrief: een 404 blijft overal rood,
# ook op het pad hieronder, dus een component dat echt verdwijnt valt nog op.
# Voeg hier alleen iets toe als de code aantoont dat `/` niet hoort te bestaan.
PROBE_PATHS='{"harvester-admin":"/health"}'

ATTEMPTS="${ATTEMPTS:-10}"
DELAY="${DELAY:-15}"

if ! echo "$URLS" | jq -e 'type == "object"' >/dev/null 2>&1; then
    echo "::error title=Productiecontrole::URLS is geen JSON-object; er valt niets te controleren."
    exit 1
fi

count=$(jq -r 'length' <<<"$URLS")
if [ "$count" -eq 0 ]; then
    echo "::error title=Productiecontrole::de deploy leverde geen enkele URL op, dus of er iets draait is niet vastgesteld."
    exit 1
fi

status=0

while IFS=$'\t' read -r naam url; do
    [ -z "$url" ] && continue
    pad=$(jq -r --arg n "$naam" '.[$n] // empty' <<<"$PROBE_PATHS")
    [ -n "$pad" ] && url="${url%/}${pad}"
    code=""
    for _ in $(seq 1 "$ATTEMPTS"); do
        code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 20 -L "$url" 2>/dev/null)
        case "$code" in
            # 2xx en 3xx: bereikbaar. 401 en 403: de dienst leeft en stuurt je
            # naar de aanmelding, wat voor de meeste van deze componenten de
            # normale uitkomst is zonder sessie.
            2?? | 3?? | 401 | 403)
                echo "  ${naam}: ${code}  ${url}"
                break
                ;;
        esac
        sleep "$DELAY"
    done

    case "$code" in
        2?? | 3?? | 401 | 403) ;;
        *)
            echo "::error title=Productiecontrole::${naam} antwoordt niet (${code:-geen verbinding}) op ${url}"
            status=1
            ;;
    esac
done < <(jq -r 'to_entries[] | "\(.key)\t\(.value)"' <<<"$URLS")

if [ "$status" -eq 0 ]; then
    echo "${count} component(en) bereikbaar."
fi
exit "$status"
