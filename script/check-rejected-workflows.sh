#!/usr/bin/env bash
# Zoekt workflowbestanden die niet draaien: afgekeurd door Actions, of
# uitgeschakeld. Een afgekeurd bestand heet naar zijn pad, want er valt geen
# `name:` te lezen uit iets dat niet is ingelezen. Zie issue #1281.
set -uo pipefail

: "${REPO:?REPO is verplicht}"

antwoord=$(gh api "repos/${REPO}/actions/workflows" --paginate \
    --jq '.workflows[] | [.name, .path, .state, .html_url] | @tsv' 2>&1)
status=$?

# Zonder lijst zegt deze controle niets, en dan is doorlaten de verkeerde
# uitkomst.
if [ "$status" -ne 0 ]; then
    echo "::error title=Workflows::kon de workflows niet opvragen, dus deze controle zegt niets: ${antwoord}"
    exit 1
fi

stil=()
while IFS=$'\t' read -r naam pad staat url; do
    [ -z "$pad" ] && continue
    if [ "$naam" = "$pad" ]; then
        stil+=("${pad} — afgekeurd door Actions — ${url}")
    elif [ "$staat" != "active" ]; then
        stil+=("${pad} — staat op ${staat} — ${url}")
    fi
done <<< "$antwoord"

if [ "${#stil[@]}" -gt 0 ]; then
    echo "::error title=Workflows::${#stil[@]} workflowbestand(en) draaien niet. Zie issue #1281."
    printf '  %s\n' "${stil[@]}"
    exit 1
fi

echo "Alle workflows zijn ingelezen en actief."
