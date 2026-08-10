#!/usr/bin/env bash
# Verwijdert een image-versie alleen als alle drie gelden: elke tag heeft de
# `sha-`-vorm, geen tag draait in ZAD, en de versie is ouder dan
# RETENTION_DAYS. Die laatste dekt de race met een deploy die doorschuift
# tussen het opvragen en het verwijderen.
#
# Productie draait op een sha-tag, dus alleen op de tagvorm afgaan zou het image
# onder de draaiende deployment vandaan halen. Lukt het opvragen van die lijst
# niet, dan verwijdert dit script niets.
#
# Versies zonder tags blijven staan: single-arch builds, dus dat is buildcache.
set -uo pipefail

: "${ORG:?ORG is verplicht}"
: "${PACKAGES:?PACKAGES is verplicht (spatiegescheiden)}"
: "${ZAD_API_KEY:?ZAD_API_KEY is verplicht}"
: "${ZAD_API_BASE:?ZAD_API_BASE is verplicht}"
: "${ZAD_PROJECT:?ZAD_PROJECT is verplicht}"

RETENTION_DAYS="${RETENTION_DAYS:-7}"
DRY_RUN="${DRY_RUN:-false}"

# Een lege lijst is niet "er draait niets"; het is ook wat een verlopen sleutel
# of een foutpagina oplevert, en dan zou elke sha-tag als ongebruikt gelden.
# Vandaar --fail-with-body en de vormcontrole hieronder.
deployments_json="$(
    curl -sS --fail-with-body --max-time 60 -H "X-API-Key: ${ZAD_API_KEY}" \
        "${ZAD_API_BASE}/v2/projects/${ZAD_PROJECT}/deployments"
)" || {
    echo "::error title=Image-opruiming::kon de draaiende deployments niet opvragen bij ZAD; er wordt niets verwijderd."
    exit 1
}

if ! jq -e 'has("deployments")' <<<"$deployments_json" >/dev/null 2>&1; then
    echo "::error title=Image-opruiming::het antwoord van ZAD bevat geen deployments-lijst; er wordt niets verwijderd."
    exit 1
fi

# De tag staat na de laatste `:` in het laatste padsegment. Op de hele URL
# zou een registry met poort of een digest-pin een niet-bestaande "tag"
# opleveren, en geen match betekent hier: mag weg.
#
# Zonder `-e`, anders geeft jq ook status non-zero als er domweg geen images
# in staan en meldt dit pad "onleesbaar" terwijl de leegte-controle hieronder
# de juiste diagnose heeft. Een echte jq-fout blijft wel non-zero.
images_raw="$(
    jq -r '.deployments[]?.components[]?.image // empty' <<<"$deployments_json"
)" || {
    echo "::error title=Image-opruiming::het antwoord van ZAD is niet te lezen als een lijst draaiende images; er wordt niets verwijderd."
    exit 1
}

in_use=()
while IFS= read -r image; do
    [ -z "$image" ] && continue
    last="${image##*/}"
    case "$last" in
        *@*)
            # Digest-gepind: de tag is niet af te leiden, dus stoppen.
            echo "::error title=Image-opruiming::ZAD rapporteert een digest-gepinde image (${image}); dit script kan dan niet vaststellen welke tag draait."
            exit 1
            ;;
        *:*) in_use+=("${last##*:}") ;;
        # Geen tag betekent latest, en die is al beschermd: niet sha-only.
        *) in_use+=("latest") ;;
    esac
done <<<"$images_raw"

if [ "${#in_use[@]}" -eq 0 ]; then
    echo "::error title=Image-opruiming::ZAD gaf geen enkele draaiende image terug. Zonder die lijst is niet vast te stellen wat veilig weg kan."
    exit 1
fi

mapfile -t in_use < <(printf '%s\n' "${in_use[@]}" | sort -u)

echo "In gebruik volgens ZAD (${#in_use[@]}): ${in_use[*]}"

is_in_use() {
    local tag="$1" used
    for used in "${in_use[@]}"; do
        [ "$tag" = "$used" ] && return 0
    done
    return 1
}

cutoff="$(date -u -d "${RETENTION_DAYS} days ago" +%s)"
deleted=0
kept_in_use=0

# --- 2. Per package langs de versies ---
for package in ${PACKAGES}; do
    versions="$(
        gh api --paginate -F per_page=100 "/orgs/${ORG}/packages/container/${package}/versions"
    )" || {
        echo "::error title=Image-opruiming::kon de versies van ${package} niet opvragen; er wordt niets verwijderd."
        exit 1
    }

    while read -r id updated tags; do
        [ -z "$id" ] && continue

        skip=false
        for tag in $tags; do
            if is_in_use "$tag"; then
                echo "  behoud ${package} ${tag} (draait in ZAD)"
                kept_in_use=$((kept_in_use + 1))
                skip=true
                break
            fi
        done
        [ "$skip" = true ] && continue

        # Een onleesbaar tijdstempel is een reden om over te slaan, niet om
        # de leeftijdstoets stil te laten wegvallen.
        if ! age="$(date -u -d "$updated" +%s 2>/dev/null)" || [ -z "$age" ]; then
            echo "::warning title=Image-opruiming::${package} versie ${id} heeft een onleesbare datum (${updated}); overgeslagen."
            continue
        fi
        [ "$age" -gt "$cutoff" ] && continue

        if [ "$DRY_RUN" = true ]; then
            echo "  zou verwijderen: ${package} ${tags} (${updated})"
            deleted=$((deleted + 1))
            continue
        fi

        if gh api --method DELETE \
            "/orgs/${ORG}/packages/container/${package}/versions/${id}"; then
            echo "  verwijderd: ${package} ${tags}"
            deleted=$((deleted + 1))
        else
            echo "::warning title=Image-opruiming::${package} versie ${id} (${tags}) kon niet worden verwijderd."
        fi
    done < <(
        jq -r '
            .[]
            | select((.metadata.container.tags | length) > 0)
            | select(.metadata.container.tags | all(startswith("sha-")))
            | "\(.id) \(.updated_at) \(.metadata.container.tags | join(" "))"
        ' <<<"$versions"
    )
done

echo "Opgeruimd: ${deleted}. Overgeslagen omdat ze draaien: ${kept_in_use}."
