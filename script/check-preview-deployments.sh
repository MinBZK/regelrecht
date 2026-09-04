#!/usr/bin/env bash
# Het spiegelbeeld van check-preview-environments.sh: geen `prN`-deployment in
# ZAD mag nog bij een gesloten pull request horen. De opruiming inventariseert
# GitHub-environments, dus wat geen environment meer heeft valt daar buiten
# beeld en blijft draaien.
#
# Deze poort telt wat er staat, niet wat er is gemeld, en draait dus ná de
# opruiming. Het verwijderen bij ZAD is asynchroon, vandaar de wachtlus.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${ZAD_API_KEY:?ZAD_API_KEY is verplicht}"
: "${ZAD_API_BASE:?ZAD_API_BASE is verplicht}"
: "${ZAD_PROJECT:?ZAD_PROJECT is verplicht}"

WAIT_SECONDS="${WAIT_SECONDS:-180}"
POLL_INTERVAL="${POLL_INTERVAL:-15}"

# Nul open PR's kan kloppen, maar het is ook wat een mislukte aanroep oplevert,
# en dan zou elk deployment hieronder ten onrechte als achtergebleven gelden.
mapfile -t open_prs < <(
    gh pr list --repo "${REPO}" --state open --limit 1000 --json number \
        --jq '.[] | "pr\(.number)"' 2>/dev/null | sort -u
)

if [ "${#open_prs[@]}" -eq 0 ]; then
    echo "::error title=Preview-opruiming::kon de open pull requests niet opvragen; de controle zegt zonder die lijst niets."
    exit 1
fi

seen=0
stale=()

# Vult `seen` en `stale`. Status 2: niet op te halen. Status 3: opgehaald maar
# geen deployments-lijst, wat als lege lijst zou doorgaan voor "niets te doen".
read_state() {
    local json previews preview open keep
    json="$(
        curl -sS --fail-with-body --max-time 60 -H "X-API-Key: ${ZAD_API_KEY}" \
            "${ZAD_API_BASE}/v2/projects/${ZAD_PROJECT}/deployments"
    )" || return 2

    # Een 200 met een foutlichaam levert een lege lijst op, en dat zou hier als
    # "niets achtergebleven" gelden.
    jq -e 'has("deployments")' <<<"$json" >/dev/null 2>&1 || return 3

    mapfile -t previews < <(
        jq -r '.deployments[]?.name // empty' <<<"$json" |
            grep -E '^pr[0-9]+$' | sort -u
    )

    seen=${#previews[@]}
    stale=()
    for preview in "${previews[@]}"; do
        keep=false
        for open in "${open_prs[@]}"; do
            [ "$preview" = "$open" ] && keep=true && break
        done
        [ "$keep" = false ] && stale+=("$preview")
    done
    # Zonder dit is de status die van de laatste `&&`, en dat leest de
    # aanroeper als een mislukte opvraging.
    return 0
}

deadline=$((SECONDS + WAIT_SECONDS))
while true; do
    read_state
    case $? in
        0) ;;
        3)
            echo "::error title=Preview-opruiming::het antwoord van ZAD bevat geen deployments-lijst; zonder die lijst zegt deze controle niets."
            exit 1
            ;;
        *)
            echo "::error title=Preview-opruiming::kon de deployments niet opvragen bij ZAD; zonder die lijst zegt deze controle niets."
            exit 1
            ;;
    esac

    [ "${#stale[@]}" -eq 0 ] && break
    [ "$SECONDS" -ge "$deadline" ] && break

    echo "nog ${#stale[@]} te gaan (${stale[*]}); opnieuw over ${POLL_INTERVAL}s"
    sleep "$POLL_INTERVAL"
done

echo "prN-deployments in ZAD: ${seen}, waarvan bij een open PR: $((seen - ${#stale[@]}))"

if [ "${#stale[@]}" -gt 0 ]; then
    echo "::error title=Preview-opruiming::${#stale[@]} ZAD-deployment(s) horen bij een gesloten pull request en draaien nog na ${WAIT_SECONDS}s. De opruiming kan hier succes voor gemeld hebben; die melding komt uit de ZAD-API en niet uit de uitkomst."
    printf '  %s\n' "${stale[@]}"
    exit 1
fi

echo "Niets achtergebleven."
