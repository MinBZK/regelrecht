#!/usr/bin/env bash
# Verwijdert `prN`-deployments in ZAD die bij een gesloten pull request horen.
# De bestaande opruiming inventariseert GitHub-environments en mist daarmee
# alles waarvan de environment al weg is; dit script kijkt andersom.
#
# check-preview-deployments.sh stelt daarna vast wat er werkelijk overblijft.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${ZAD_API_KEY:?ZAD_API_KEY is verplicht}"
: "${ZAD_API_BASE:?ZAD_API_BASE is verplicht}"
: "${ZAD_PROJECT:?ZAD_PROJECT is verplicht}"

DRY_RUN="${DRY_RUN:-false}"

deployments_json="$(
    curl -sS --fail-with-body --max-time 60 -H "X-API-Key: ${ZAD_API_KEY}" \
        "${ZAD_API_BASE}/v2/projects/${ZAD_PROJECT}/deployments"
)" || {
    echo "::error title=Preview-opruiming::kon de deployments niet opvragen bij ZAD."
    exit 1
}

# Een 200 met een foutlichaam levert een lege lijst op, en dat zou hier als
# "niets te doen" gelden.
if ! jq -e 'has("deployments")' <<<"$deployments_json" >/dev/null 2>&1; then
    echo "::error title=Preview-opruiming::het antwoord van ZAD bevat geen deployments-lijst."
    exit 1
fi

mapfile -t previews < <(
    jq -r '.deployments[]?.name // empty' <<<"$deployments_json" |
        grep -E '^pr[0-9]+$' | sort -u
)

if [ "${#previews[@]}" -eq 0 ]; then
    echo "Geen prN-deployments in ZAD."
    exit 0
fi

# Nul open PR's kan kloppen, maar het is ook wat een mislukte aanroep oplevert.
# Zonder die lijst zou dit script elk preview-deployment verwijderen, inclusief
# die van pull requests waar iemand op dat moment naar kijkt.
mapfile -t open_prs < <(
    gh pr list --repo "${REPO}" --state open --limit 1000 --json number \
        --jq '.[] | "pr\(.number)"' 2>/dev/null | sort -u
)

if [ "${#open_prs[@]}" -eq 0 ]; then
    echo "::error title=Preview-opruiming::kon de open pull requests niet opvragen; er wordt niets verwijderd."
    exit 1
fi

removed=0
for preview in "${previews[@]}"; do
    keep=false
    for open in "${open_prs[@]}"; do
        [ "$preview" = "$open" ] && keep=true && break
    done
    [ "$keep" = true ] && continue

    if [ "$DRY_RUN" = true ]; then
        echo "  zou verwijderen: ${preview}"
        removed=$((removed + 1))
        continue
    fi

    # De padvorm wijkt af van die bij het lezen: geen `/deployments/` ertussen.
    # Overgenomen uit `ZadClient.delete_deployment` in zad-cli 0.9.1.
    if curl -sS --fail-with-body --max-time 120 -X DELETE \
        -H "X-API-Key: ${ZAD_API_KEY}" \
        "${ZAD_API_BASE}/v2/projects/${ZAD_PROJECT}/${preview}" >/dev/null; then
        echo "  verwijdering aangevraagd: ${preview}"
        removed=$((removed + 1))
    else
        echo "::warning title=Preview-opruiming::${preview} kon niet worden verwijderd."
    fi
done

echo "Verweesde deployments opgeruimd: ${removed}."
