#!/usr/bin/env bash
# Controleert wat de nachtelijke opruiming heeft achtergelaten: geen enkele
# `prN`-environment mag nog bij een gesloten pull request horen.
#
# De opruimactie meldt zelf succes op basis van alleen de ZAD-kant. In
# `zad-actions/scheduled-cleanup` zet stap 1 `ENV_SUCCESS`, en de stappen die
# de GitHub-environment, de deployments en de images verwijderen raken die
# variabele nergens aan. De run van 7 augustus 2026 sloot daardoor af met
# "Successfully cleaned 773 environment(s)" terwijl alle 773 verwijderingen met
# 404 waren gefaald, en dat stond verstopt in een log van 4351 regels.
#
# Deze poort telt de uitkomst in plaats van de melding, en hij hoort dus na de
# opruiming te draaien en niet ervoor. Zie issue #1213 voor de oorzaak: het
# admin-token mist repo-admin, en `DELETE /repos/{owner}/{repo}/environments/…`
# antwoordt dan met 404 in plaats van 403.
set -uo pipefail

: "${REPO:?REPO is verplicht}"

# Alle environments met de naamvorm die de previews gebruiken.
mapfile -t envs < <(
    gh api "repos/${REPO}/environments" --paginate \
        --jq '.environments[].name' 2>/dev/null | grep -E '^pr[0-9]+$' | sort -u
)

# Eén aanroep in plaats van één per environment. Open PR's zijn precies degene
# waarvan de preview mag blijven staan.
mapfile -t open_prs < <(
    gh pr list --repo "${REPO}" --state open --limit 1000 --json number \
        --jq '.[] | "pr\(.number)"' 2>/dev/null | sort -u
)

if [ "${#open_prs[@]}" -eq 0 ]; then
    # Nul open PR's kan kloppen, maar het is ook wat een mislukte aanroep
    # oplevert, en dan zou elke environment hieronder ten onrechte als
    # achtergebleven gelden.
    echo "::error title=Preview-opruiming::kon de open pull requests niet opvragen; de controle zegt zonder die lijst niets."
    exit 1
fi

stale=()
for env in "${envs[@]}"; do
    keep=false
    for open in "${open_prs[@]}"; do
        [ "$env" = "$open" ] && keep=true && break
    done
    [ "$keep" = false ] && stale+=("$env")
done

echo "prN-environments: ${#envs[@]}, waarvan bij een open PR: $((${#envs[@]} - ${#stale[@]}))"

if [ "${#stale[@]}" -gt 0 ]; then
    echo "::error title=Preview-opruiming::${#stale[@]} environment(s) horen bij een gesloten pull request en staan er nog. De opruimstap hiervoor meldt succes op basis van alleen de ZAD-kant, dus een groene job zegt hier niets over. Zie issue #1213."
    printf '  %s\n' "${stale[@]}"
    exit 1
fi

echo "Niets achtergebleven."
