#!/usr/bin/env bash
# Deploy-poort: blokkeer tot de CI van déze commit klaar is, en laat alleen
# groen door.
#
# `ci.yml` en `deploy.yml` hangen allebei aan `push` op main en weten niets van
# elkaar. Op 7 augustus 2026 faalde CI op commit 4518c575 (Security Audit) en
# ging deploy-production diezelfde minuut gewoon door. Er is geen `needs` tussen
# twee workflows, dus die koppeling moet hiervandaan komen.
#
# De poort leest de CI-run op naam en head-SHA, niet op run-id: deploy kent het
# run-id van CI niet, en beide runs starten uit dezelfde push. Dat betekent wel
# dat er meerdere runs kunnen staan (een handmatige rerun, een `workflow_dispatch`);
# de nieuwste telt, want dat is de laatste uitspraak over deze commit.
#
# Afwezig is niet hetzelfde als geslaagd. Staat er voor deze SHA geen CI-run,
# dan blokkeert de poort in plaats van door te rollen: dat is precies het geval
# waarin niemand iets heeft nagekeken.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${SHA:?SHA is verplicht}"

WORKFLOW="${WORKFLOW:-CI}"
MAX_WAIT_SECONDS="${MAX_WAIT_SECONDS:-2400}"
POLL_SECONDS="${POLL_SECONDS:-20}"

fail() {
    echo "::error title=Deploy-poort::$1"
    exit 1
}

# Foutuitvoer van de laatste API-aanroep. Een bestand en geen variabele: de
# aanroep hieronder staat in een command-substitution, dus een variabele die de
# functie zet gaat met die subshell verloren.
ERRFILE=$(mktemp)
trap 'rm -f "$ERRFILE"' EXIT

# Status en conclusie van de nieuwste run van $WORKFLOW voor deze commit. Leeg
# wanneer er geen run is; exitcode 1 wanneer de aanroep zelf mislukte. Dat
# onderscheid is nodig: een tokenfout, rate limit of netwerkstoring mag niet als
# "er is geen run" gelezen worden, want dan noemt de poort een oorzaak die hij
# niet getoetst heeft.
latest_run() {
    gh api "repos/${REPO}/actions/runs?head_sha=${SHA}&per_page=100" \
        --jq "[.workflow_runs[] | select(.name == \"${WORKFLOW}\")]
              | sort_by(.run_started_at) | last
              | select(. != null)
              | \"\(.status)\t\(.conclusion // \"\")\t\(.html_url)\"" 2>"$ERRFILE"
}

waited=0
while :; do
    if ! run="$(latest_run)"; then
        melding=$(tr '\n' ' ' <"$ERRFILE")
        # Blijven proberen zolang er tijd is: een rate limit trekt vanzelf bij.
        # Blijft het mislukken, dan is dat de melding, en niet "geen run".
        if [ "$waited" -ge "$MAX_WAIT_SECONDS" ]; then
            fail "de GitHub-API blijft fouten geven, dus of CI geslaagd is voor ${SHA} is niet vast te stellen: ${melding}"
        fi
        echo "API-fout, opnieuw proberen: ${melding}"
        sleep "$POLL_SECONDS"
        waited=$((waited + POLL_SECONDS))
        continue
    fi

    if [ -z "$run" ]; then
        # Kan een race zijn met het aanmaken van de run, dus even wachten mag.
        if [ "$waited" -ge "$MAX_WAIT_SECONDS" ]; then
            fail "geen enkele ${WORKFLOW}-run gevonden voor ${SHA} binnen ${MAX_WAIT_SECONDS}s. Zonder run is er niets nagekeken; er gaat niets naar productie."
        fi
    else
        IFS=$'\t' read -r status conclusion url <<<"$run"

        if [ "$status" = "completed" ]; then
            case "$conclusion" in
                success)
                    echo "${WORKFLOW} voor ${SHA} is geslaagd: ${url}"
                    exit 0
                    ;;
                # Een overgeslagen CI-run betekent dat de workflow niet van
                # toepassing was, niet dat het werk goedgekeurd is.
                *)
                    fail "${WORKFLOW} voor ${SHA} eindigde als '${conclusion}', dus er gaat niets naar productie. Zie ${url}"
                    ;;
            esac
        fi

        echo "${WORKFLOW} voor ${SHA} is '${status}', nog $((MAX_WAIT_SECONDS - waited))s geduld: ${url}"
    fi

    if [ "$waited" -ge "$MAX_WAIT_SECONDS" ]; then
        fail "${WORKFLOW} voor ${SHA} was na ${MAX_WAIT_SECONDS}s nog niet klaar. Draai deze job opnieuw zodra CI af is."
    fi

    sleep "$POLL_SECONDS"
    waited=$((waited + POLL_SECONDS))
done
