#!/usr/bin/env bash
# Meet de doorlooptijd van de CI op één pull request en spuugt het
# `ciWorkflows`-blok uit dat docs/src/lib/ci-pipeline.ts verwacht.
#
# Bijwerken van de pagina /operations/ci-doorlooptijd:
#   just meet-ci > /tmp/ci.ts
# en dan het `ciWorkflows`-blok in docs/src/lib/ci-pipeline.ts vervangen door
# de inhoud van dat bestand, plus `measuredOn` op vandaag zetten.
#
# Meetwijze (moet gelijk blijven aan wat de pagina zegt): medianen over de
# laatste N afgeronde runs van het type pull_request. Wachttijd is gerekend
# vanaf het aanmaken van de run, niet vanaf het starten van de job, zodat de
# looptijd van voorgangers in de wachtbalk zichtbaar blijft. Overgeslagen jobs
# tellen niet mee.
set -euo pipefail

REPO="${REPO:-MinBZK/regelrecht}"
RUNS="${RUNS:-60}"

# Workflows met een eigen groep in de grafiek, in deze volgorde. Alles wat hier
# niet in staat komt onder "Overig" terecht.
WORKFLOW_GROUPS=("CI" "Build and Deploy" "Claude Code Review")

# Poorten: jobs die zelf niets doen en via `needs` op andere jobs wachten. Dit
# is een leesbeslissing over de workflow, niet iets wat de API vertelt.
GATES=("Test" "deploy-preview" "Claude review completed")

for cmd in gh jq; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "FOUT: $cmd is nodig maar niet geïnstalleerd" >&2
        exit 1
    fi
done

echo "Runs ophalen van $REPO ..." >&2
# Pagineren met de hand: `gh api --paginate` haalt alles op wat er is, en dat
# zijn er duizenden.
runs_json='[]'
page=1
while [ "$(jq 'length' <<<"$runs_json")" -lt "$RUNS" ]; do
    page_json=$(gh api \
        "repos/$REPO/actions/runs?event=pull_request&status=completed&per_page=100&page=$page" \
        --jq '[.workflow_runs[] | {id, name, created_at, updated_at}]')
    [ "$(jq 'length' <<<"$page_json")" -eq 0 ] && break
    runs_json=$(jq -s 'add' <<<"$runs_json"$'\n'"$page_json")
    page=$((page + 1))
done
runs_json=$(jq ".[0:$RUNS]" <<<"$runs_json")

run_count=$(jq 'length' <<<"$runs_json")
if [ "$run_count" -eq 0 ]; then
    echo "FOUT: geen afgeronde pull_request-runs gevonden" >&2
    exit 1
fi
echo "$run_count runs, jobs ophalen ..." >&2

# Per job één regel JSON. `started_at` van een job is het moment dat de runner
# hem oppakt; het verschil met `created_at` van de run is dus de wachttijd
# inclusief de looptijd van alles waar hij via `needs` op wacht.
jobs_json=$(
    jq -r '.[] | "\(.id)\t\(.name)\t\(.created_at)"' <<<"$runs_json" |
        while IFS=$'\t' read -r run_id wf_name created_at; do
            # </dev/null: zonder dat leest gh de resterende regels van de
            # while-lus op en blijft er één run over.
            gh api --paginate "repos/$REPO/actions/runs/$run_id/jobs?per_page=100" \
                --jq '.jobs[]' </dev/null |
                jq -c --arg wf "$wf_name" --arg created "$created_at" '
                    select(.conclusion != "skipped")
                    | select(.started_at != null and .completed_at != null)
                    | { workflow: $wf,
                        job: .name,
                        wait: ((.started_at | fromdateiso8601) - ($created | fromdateiso8601)),
                        work: ((.completed_at | fromdateiso8601) - (.started_at | fromdateiso8601)) }'
        done | jq -s '.'
)

# De verplichte checks komen uit de branch protection van main. Die endpoint
# vereist admin-rechten; zonder die rechten valt het script terug op wat de
# pagina nu zegt, met een waarschuwing zodat het opvalt.
required_json=$(gh api "repos/$REPO/branches/main/protection/required_status_checks" \
    --jq '[.contexts[]]' 2>/dev/null || true)
if [ -z "$required_json" ]; then
    echo "LET OP: branch protection niet leesbaar (admin-recht nodig); terugval op de bekende lijst" >&2
    required_json='["Pre-commit","WASM Build","Protect schema versions","Security Audit","Test"]'
fi

groups_json=$(printf '%s\n' "${WORKFLOW_GROUPS[@]}" | jq -R . | jq -s '.')
gates_json=$(printf '%s\n' "${GATES[@]}" | jq -R . | jq -s '.')

# Het jq-programma staat in een quoted heredoc: het bevat zelf enkele
# aanhalingstekens (de TypeScript-strings die het uitspuugt).
jq_program=$(cat <<'JQ'
def median: sort
    | (length / 2 | floor) as $m
    | if length == 0 then null
      elif length % 2 == 1 then .[$m]
      else (.[$m - 1] + .[$m]) / 2 end;
def minutes: (. / 60 * 10 | round) / 10;
def num: tostring | if test("[.]") then . else . + ".0" end;

. as $jobs
| ($jobs | map(.workflow) | unique) as $seen
| ($groups + (($seen - $groups) | sort)) as $ordered
| [ $ordered[]
    | . as $wf
    | ($jobs | map(select(.workflow == $wf))) as $rows
    | select(($rows | length) > 0)
    | { workflow: $wf,
        total: ($runs
                | map(select(.name == $wf)
                      | (.updated_at | fromdateiso8601) - (.created_at | fromdateiso8601))
                | median
                | if . == null then null else minutes end),
        jobs: ($rows
               | group_by(.job)
               | map({ name: .[0].job,
                       wait: (map(.wait) | median | minutes),
                       work: (map(.work) | median | minutes) })
               | sort_by(.wait)) }
  ]
| ( [ .[] | select(.workflow | IN($groups[])) ]
    + ( [ .[] | select(.workflow | IN($groups[]) | not) | .jobs ]
        | flatten
        | if length == 0 then []
          else [{ workflow: "Overig", total: null, jobs: sort_by(.wait) }] end ) )
| "export const ciWorkflows: CiWorkflow[] = [",
  ( .[]
    | "  {",
      "    name: '\(.workflow)',",
      "    total: \(if .total == null then "null" else (.total | num) end),",
      "    jobs: [",
      ( .jobs[]
        | "      { name: '\(.name)', wait: \(.wait | num), work: \(.work | num)"
          + (if (.name | IN($required[])) then ", required: true" else "" end)
          + (if (.name | IN($gates[])) then ", gate: true" else "" end)
          + " },"
      ),
      "    ],",
      "  },"
  ),
  "];"
JQ
)

jq -r \
    --argjson runs "$runs_json" \
    --argjson groups "$groups_json" \
    --argjson gates "$gates_json" \
    --argjson required "$required_json" \
    "$jq_program" <<<"$jobs_json"
