#!/usr/bin/env bash
# Haalt één uitvoering van de CI op en spuugt de blokken uit die
# docs/src/lib/ci-pipeline.ts verwacht: `ciRun`, `ciScaleMinutes` en
# `ciWorkflows`.
#
# Bijwerken van de pagina /operations/ci-doorlooptijd:
#   just meet-ci > /tmp/ci.ts          # nieuwste volledig geslaagde run
#   just meet-ci afa1408c > /tmp/ci.ts # een specifieke commit
# en dan de drie blokken in docs/src/lib/ci-pipeline.ts vervangen.
#
# Eén run en geen gemiddelde: medianen per job komen uit verschillende
# uitvoeringen, en met `needs`-pijlen ertussen levert dat een schema op dat
# nergens zo gelopen heeft. Nul op de tijdas is het vroegste aanmaakmoment van
# de runs bij deze commit; `start` en `end` van elke job zijn gerekend vanaf
# daar. De runnerwachttijd staat er niet in: de pagina leidt hem af uit het
# einde van de laatste `needs`-voorganger.
#
# De jobs-API geeft de `needs`-graaf niet mee, dus die wordt uit de
# workflow-bestanden gelezen. Zonder die graaf zijn de pijlen niet te tekenen.
# Matrix-jobs (`name: Rust tests (${{ matrix.leg }})`) worden op patroon
# gematcht, zodat beide benen bij hun definitie horen.
#
# Wat het script niet meelevert is redactioneel: de `note` naast een
# workflownaam.
set -euo pipefail

cd "$(dirname "$0")/.."

REPO="${REPO:-MinBZK/regelrecht}"
SHA="${1:-}"

# Workflows met een eigen groep in de grafiek, in deze volgorde. Alles wat hier
# niet in staat komt onder "Overig" terecht.
WORKFLOW_GROUPS=("CI" "Build and Deploy" "Claude Code Review")

# Poorten: jobs die zelf niets doen en alleen de uitkomst van andere jobs lezen.
# Dat is een leesbeslissing over de workflow, niet iets wat de API zegt.
GATES=("Test" "deploy-preview" "Claude review completed")

for cmd in gh jq awk; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "FOUT: $cmd is nodig maar niet geïnstalleerd" >&2
        exit 1
    fi
done

# --- needs-graaf uit de workflow-bestanden ---------------------------------
# De vorm van een workflow-bestand is vast: een job-sleutel op twee spaties,
# zijn eigenschappen op vier. Alleen `name` en `needs` zijn hier interessant.
graph_json=$(
    for wf in .github/workflows/*.yml .github/workflows/*.yaml; do
        [ -e "$wf" ] || continue
        awk -v path="$wf" '
            function esc(s) { gsub(/\\/, "\\\\", s); gsub(/"/, "\\\"", s); return s }
            function trim(s) { sub(/^[ \t]+/, "", s); sub(/[ \t\r]+$/, "", s); return s }
            function unquote(s,   q) {
                s = trim(s)
                q = substr(s, 1, 1)
                if ((q == "\"" || q == "'"'"'") && substr(s, length(s), 1) == q)
                    s = substr(s, 2, length(s) - 2)
                return s
            }
            function flush(   n, i, parts, v, first) {
                if (id == "") return
                printf "{\"path\":\"%s\",\"id\":\"%s\",\"name\":\"%s\",\"needs\":[",
                       esc(path), esc(id), esc(name == "" ? id : name)
                n = split(needs, parts, ",")
                first = 1
                for (i = 1; i <= n; i++) {
                    v = unquote(parts[i])
                    if (v == "") continue
                    printf "%s\"%s\"", (first ? "" : ","), esc(v)
                    first = 0
                }
                print "]}"
                id = ""; name = ""; needs = ""
            }
            /^jobs:[ \t]*$/ { injobs = 1; next }
            /^[^ \t#]/ { if (injobs) { flush(); injobs = 0 } }
            !injobs { next }
            /^  [A-Za-z0-9_-]+:[ \t]*$/ {
                flush()
                id = $0; sub(/^  /, "", id); sub(/:[ \t]*$/, "", id)
                next
            }
            /^    name:/ { s = $0; sub(/^    name:/, "", s); name = unquote(s); next }
            /^    needs:/ {
                s = $0; sub(/^    needs:/, "", s); s = trim(s)
                gsub(/^\[|\]$/, "", s)
                needs = s
                next
            }
            END { flush() }
        ' "$wf"
    done | jq -s '.'
)

if [ "$(jq 'length' <<<"$graph_json")" -eq 0 ]; then
    echo "FOUT: geen jobs gevonden in .github/workflows" >&2
    exit 1
fi

# --- de run kiezen ----------------------------------------------------------
recent=$(gh api \
    "repos/$REPO/actions/runs?event=pull_request&status=completed&per_page=100" \
    --jq '[.workflow_runs[] | {id, name, path, head_sha, conclusion, created_at,
                               pr: (.pull_requests[0].number // null)}]')

if [ -z "$SHA" ]; then
    # Nieuwste commit waarvan alle pull_request-runs geslaagd zijn en waar de
    # drie hoofdworkflows in zitten; anders staan er gaten in de plaat.
    SHA=$(jq -r --argjson groups "$(printf '%s\n' "${WORKFLOW_GROUPS[@]}" | jq -R . | jq -s '.')" '
        group_by(.head_sha)
        | map(select(($groups - [.[].name]) | length == 0))
        | map(select(all(.[] | select(.name | IN($groups[])); .conclusion == "success")))
        | sort_by(.[0].created_at)
        | reverse
        | .[0][0].head_sha // ""
    ' <<<"$recent")
fi

if [ -z "$SHA" ]; then
    echo "FOUT: geen commit gevonden waar ${WORKFLOW_GROUPS[*]} alle drie geslaagd zijn" >&2
    exit 1
fi

# Bij een re-run staan er twee runs van dezelfde workflow op één commit; alleen
# de nieuwste telt, anders komt elke job dubbel in de plaat.
runs_json=$(jq --arg sha "$SHA" '
    [.[] | select(.head_sha | startswith($sha))]
    | group_by(.name)
    | map(sort_by(.created_at) | last)
    | sort_by(.created_at)
' <<<"$recent")
if [ "$(jq 'length' <<<"$runs_json")" -eq 0 ]; then
    echo "FOUT: geen afgeronde pull_request-runs gevonden voor $SHA" >&2
    exit 1
fi

failed=$(jq -r '[.[] | select(.conclusion != "success") | .name] | join(", ")' <<<"$runs_json")
if [ -n "$failed" ]; then
    echo "LET OP: niet alles is groen bij $SHA ($failed); de plaat toont een run met een rode workflow" >&2
fi

echo "Run bij $SHA: $(jq -r 'length' <<<"$runs_json") workflows, jobs ophalen ..." >&2

runs_with_jobs=$(
    jq -r '.[] | "\(.id)\t\(.name)\t\(.path)\t\(.created_at)"' <<<"$runs_json" |
        while IFS=$'\t' read -r run_id wf_name wf_path created; do
            # </dev/null: zonder dat leest gh de resterende regels van de
            # while-lus op en blijft er één run over.
            gh api --paginate "repos/$REPO/actions/runs/$run_id/jobs?per_page=100" \
                --jq '[.jobs[] | {name, conclusion, started_at, completed_at}]' </dev/null |
                jq -c -s --arg wf "$wf_name" --arg path "$wf_path" --arg created "$created" \
                    '{ workflow: $wf, path: $path, created: $created,
                       jobs: (add | map(select(.started_at != null and .completed_at != null))) }'
        done | jq -s '.'
)

# --- verplichte checks ------------------------------------------------------
# Die staan in de branch protection van main. Dat endpoint vereist
# admin-rechten; zonder die rechten valt het script terug op wat de pagina nu
# zegt, met een waarschuwing zodat het opvalt.
required_json=$(gh api "repos/$REPO/branches/main/protection/required_status_checks" \
    --jq '[.contexts[]]' 2>/dev/null || true)
if [ -z "$required_json" ]; then
    echo "LET OP: branch protection niet leesbaar (admin-recht nodig); terugval op de bekende lijst" >&2
    required_json='["Pre-commit","WASM Build","Protect schema versions","Security Audit","Test"]'
fi

groups_json=$(printf '%s\n' "${WORKFLOW_GROUPS[@]}" | jq -R . | jq -s '.')
gates_json=$(printf '%s\n' "${GATES[@]}" | jq -R . | jq -s '.')
pr_number=$(jq -r '[.[].pr] | map(select(. != null)) | first // "null"' <<<"$runs_json")

# Het jq-programma staat in een quoted heredoc: het bevat zelf enkele
# aanhalingstekens (de TypeScript-strings die het uitspuugt).
jq_program=$(cat <<'JQ'
def minutes: (if . < 0 then 0 else . end) | (. * 100 | round) / 100;
def num: tostring | if test("[.]") then . else . + ".0" end;
def secs: fromdateiso8601;
def slug: ascii_downcase | gsub("[^a-z0-9]+"; "-") | gsub("^-|-$"; "");

# Een jobnaam in een workflow-bestand kan een expressie bevatten
# (`Rust tests (${{ matrix.leg }})`). Die wordt een wildcard; de rest van de
# naam wordt letterlijk gematcht.
def to_regex:
    gsub("\\$\\{\\{[^}]*\\}\\}"; "")
    | gsub("(?<c>[.\\[\\]{}()*+?^$|\\\\])"; "\\\(.c)")
    | gsub(""; ".*")
    | "^" + . + "$";

($graph | map(. + { regex: (.name | to_regex) })) as $defs
| ([.[] | .created | secs] | min) as $t0

# Eén regel per job: wanneer hij begon en wanneer hij klaar was.
| [ .[]
    | . as $run
    | ($defs | map(select(.path == $run.path))) as $wfdefs
    | $run.jobs[]
    | . as $job
    | select($job.conclusion != "skipped")
    | ($wfdefs | map(.regex as $re | select($job.name | test($re))) | first) as $def
    | { workflow: $run.workflow,
        id: ($job.name | slug),
        name: $job.name,
        defid: ($def.id // $job.name),
        needs: ($def.needs // []),
        start: ((($job.started_at | secs) - $t0) / 60 | minutes),
        end: ((($job.completed_at | secs) - $t0) / 60 | minutes) }
  ] as $rows

| ($rows | map(.workflow) | unique) as $seen
| ($groups + (($seen - $groups) | sort)) as $ordered
| [ $ordered[]
    | . as $wf
    | ($rows | map(select(.workflow == $wf)) | sort_by(.start)) as $jobs
    | select(($jobs | length) > 0)
    | { workflow: $wf, jobs: $jobs } ]
| ( [ .[] | select(.workflow | IN($groups[])) ]
    + ( [ .[] | select(.workflow | IN($groups[]) | not) | .jobs ]
        | flatten
        | if length == 0 then []
          else [{ workflow: "Overig", jobs: sort_by(.start) }] end ) )

# Pijlen: elke `needs`-relatie tussen twee jobs die allebei in deze groep staan.
# Een voorganger met meer dan drie opvolgers wordt als spine getekend.
| map(.jobs as $jobs
      | . + { edges: ( [ $jobs[] as $j
                         | $j.needs[] as $n
                         | ($jobs[] | select(.defid == $n)) as $p
                         | { from: $p.id, to: $j.id } ]
                       | group_by(.from)
                       | map({ from: .[0].from, to: map(.to), spine: (length > 3) })) })
| . as $out
| ([ $out[].jobs[].end ] | max) as $last

| "export const ciRun = {",
  "  commit: '\($sha[0:8])',",
  "  pullRequest: \($pr),",
  "  startedAt: '\($started)',",
  "  repository: '\($repo)',",
  "} as const;",
  "",
  "export const ciScaleMinutes = \(($last + 0.85) * 10 | ceil | . / 10 | num);",
  "",
  "export const ciWorkflows: CiWorkflow[] = [",
  ( $out[]
    | "  {",
      "    name: '\(.workflow)',",
      "    jobs: [",
      ( .jobs[]
        | "      { id: '\(.id)', name: '\(.name)', start: \(.start | num), end: \(.end | num)"
          + (if (.name | IN($required[])) then ", required: true" else "" end)
          + (if (.name | IN($gates[])) then ", gate: true" else "" end)
          + " },"
      ),
      "    ],",
      "    edges: [",
      ( .edges[]
        | "      { from: '\(.from)', to: ['\(.to | join("', '"))']"
          + (if .spine then ", spine: true" else "" end)
          + " },"
      ),
      "    ],",
      "  },"
  ),
  "];"
JQ
)

started_at=$(jq -r '[.[].created_at] | min' <<<"$runs_json")
# Nederlandse maandnaam met de hand: de container heeft geen nl_NL-locale.
maanden=(januari februari maart april mei juni juli augustus september oktober november december)
started_human=$(
    d=$(date -u -d "$started_at" +'%-d') || d=""
    if [ -n "$d" ]; then
        m=$(date -u -d "$started_at" +'%-m')
        printf '%s %s %s om %s UTC' \
            "$d" "${maanden[$((m - 1))]}" \
            "$(date -u -d "$started_at" +'%Y')" \
            "$(date -u -d "$started_at" +'%H:%M')"
    else
        printf '%s' "$started_at"
    fi
)

jq -r \
    --argjson graph "$graph_json" \
    --argjson groups "$groups_json" \
    --argjson gates "$gates_json" \
    --argjson required "$required_json" \
    --arg sha "$SHA" \
    --arg pr "$pr_number" \
    --arg started "$started_human" \
    --arg repo "$REPO" \
    "$jq_program" <<<"$runs_with_jobs"
