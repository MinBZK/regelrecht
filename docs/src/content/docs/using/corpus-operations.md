---
title: "Operating the Corpus"
description: "For corpus operators: harvesting laws into the corpus from the editor, dealing with failed and exhausted laws, enrichment, and reading Grafana and the daily summary."
---

This guide is for the person who keeps the corpus filled: starting harvests, following up on laws that fail, starting enrichment, and watching the pipeline in Grafana. The corpus-harvesting section lives inside the editor at `editor.regelrecht.rijks.app`. In the code and in some docs it is called Corpusinwinning; in the editor itself it is labeled "Harvester". For how the pieces behind it work, see [Harvester Admin](/components/admin), [Pipeline](/components/pipeline) and [Grafana Monitoring](/components/grafana).

Most labels in this section of the editor are English. They are quoted below as they appear.

## Before you start

You need one of the roles `harvester-reader`, `harvester-writer`, `harvester-admin` or `regelrecht-admin`. Which one decides what you can do:

| Role | What it adds |
|------|--------------|
| `harvester-reader` | Viewing every tab |
| `harvester-writer` | Starting harvest and enrich jobs |
| `harvester-admin` | Resetting an exhausted law |

A higher role includes the lower ones. The menu shows every action to everyone who can open the section; the server refuses the ones your role does not cover. See [Authentication & Roles](/auth-and-roles) for how roles are granted.

## Open the section

1. Sign in to the editor.
2. Open the account menu (the "Account" button in the top bar) and choose "Harvester".
3. The section opens on the "Overzicht" tab at `/harvesting/overview`. "Terug" returns you to the page you came from.

If "Harvester" is missing from the menu, your account has none of the roles above.

The section has five tabs:

| Tab | Address | What it shows |
|-----|---------|---------------|
| "Overzicht" | `/harvesting/overview` | Totals per job type, a chart per day, and recent failed jobs |
| "Wetten" | `/harvesting/law-entries` | Every law the pipeline knows, with its status and coverage |
| "Taken" | `/harvesting/jobs` | The jobs, grouped per law or as a flat list |
| "Markeringen" | `/harvesting/markings` | What enrichment could not express in the law format |
| "Untranslatables" | `/harvesting/untranslatables` | The same, for laws still on schema v0.5.x |

Lists and the overview refresh every 20 seconds on their own.

## Start a harvest

A harvest downloads a law from the source and converts it to YAML.

1. Click "Nieuwe harvest-job" (the plus button in the header). A sheet "New harvest job" opens.
2. Under "Law ID", enter a BWB id (`BWBR0018451`) or a CVDR id (`CVDR681386`).
3. Click "Add harvest job". Use "Add and add another" to keep the sheet open for the next id.

The job goes into the queue with normal priority. The sheet has no field for a date or a priority; those can only be set through the API.

If a harvest for the same law is already waiting or running, the sheet says "A harvest job for this law is already pending or processing." Nothing is added twice.

To harvest a law that is already in the list again, use the row menu instead:

1. Go to "Wetten".
2. Click the "Actions" button (the three dots) on the law's row.
3. Choose "Harvest".

## Follow a law or a job

On "Wetten", the "Status" column shows where each law stands. Use the "Status: All" filter to narrow the list, for example to "Harvest failed".

| Status | Meaning |
|--------|---------|
| Queued | A harvest is waiting |
| Harvesting / Enriching | A worker is on it |
| Harvested | The text is in the corpus, without machine-readable logic |
| Enriched | Enrichment finished |
| Harvest failed / Enrich failed | The last job failed; the pipeline will try again |
| Harvest exhausted / Enrich exhausted | The pipeline has given up; see below |
| Not harvestable | The source has no consolidated text for this law. This is final and not an error |

The "Coverage" column is the share of the law's articles that have a machine-readable part.

To see the jobs behind a law, choose "View job details" in its row menu. That opens "Taken" filtered on the law, with the sheet "Jobs for …" listing them. Click a job for "Job details": its attempts, its timestamps, and under "Error" the reason it failed.

## Failed and exhausted laws

The pipeline retries on two levels, and it helps to know which one you are looking at.

**A single job** gets three attempts. After a failed attempt it goes back into the queue with a growing delay (30 seconds, then a minute, up to 15 minutes). Only when the third attempt fails is the job itself "failed". In "Job details", "Attempts" shows this as `3 / 3`.

**A law** keeps a count of jobs that failed that way. After each failed job the law gets the status "Harvest failed" (or "Enrich failed") and the pipeline schedules a new job by itself. After ten failed jobs in a row the law becomes "Harvest exhausted" (or "Enrich exhausted"). A successful run sets the count back to zero.

Enrichment has one shortcut: when the model produces no machine-readable part at all, or output that does not validate, the law is exhausted straight away. Trying again would give the same result.

An exhausted law is left alone. No automatic job picks it up, and a new harvest or enrich request for it is refused. That is deliberate: something is wrong that retrying does not fix.

There is no retry button. Retrying is automatic, and starting a job by hand ("Harvest" or "Enrich" in the row menu) is the manual retry.

### Reset an exhausted law

Reset only after you have found and fixed the cause, for example a source that was down or a pipeline bug that has since been fixed. Otherwise the law will exhaust again.

1. Go to "Wetten" and filter on "Harvest exhausted" or "Enrich exhausted".
2. Read the error in "View job details" and deal with the cause.
3. In the row menu, choose "Reset exhausted". This needs `harvester-admin`.
4. The status goes back to "Harvest failed" (or "Enrich failed") and the count to zero. The reset does not start a job.
5. In the same row menu, choose "Harvest" or "Enrich" to start one.

If the reset fails, the editor shows "Reset failed:" with the reason.

## Start enrichment

Enrichment has a language model write the machine-readable part of a law that has been harvested. It does not run by itself after a harvest unless the deployment has been configured to; normally you start it.

1. Go to "Wetten". "Enrich" is only in the row menu when the law is "Harvested", "Enriched" or "Enrich failed".
2. Choose "Enrich" in the row menu.
3. The editor confirms with "Created 2 enrich job(s) for …": one job per model provider (`opencode` and `claude`), so the outputs can be compared.

If jobs for that law are still waiting or running, the editor says "Enrich jobs for this law are already pending or processing."

Enrichment is rate-limited per hour. When the limit is reached, or the deployment has it set to zero, enrich jobs stay "Pending" until capacity frees up. A pile of pending enrich jobs is therefore not by itself a fault.

## Review markings

When enrichment meets a construct the law format cannot express, it records a marking instead of guessing. The markings are read-only in this section.

1. Open "Markeringen".
2. The card "Wat het formaat mist" groups markings by the change to the format that would resolve them, the most widespread first. Click a group to filter the list below it.
3. Filter further on "Wet", "Constructie", "Soort", "Provider" or "Beoordeeld".
4. Click a row for "Markering in detail": what could not be expressed, what would resolve it, and the legal text it hangs on. "Blokkeert" says whether the marking stops the article from working.

"Soort" separates a missing operation ("bewerking") from a missing form in the format ("formaat"). See [Markings](/concepts/markings) for the concept.

"Untranslatables" shows the older form of the same information, for laws that are still on schema v0.5.x. It works the same way, with the detail sheet "Untranslatable details".

## Read Grafana

Grafana is at `grafana.regelrecht.rijks.app`. Sign in with your RegelRecht account; members of the `grafana-admin` group can edit, everyone else can view. Open the dashboard "Regelrecht Overview" in the folder "Regelrecht". It shows the last 24 hours in Amsterdam time by default.

| Panel | What to look at |
|-------|-----------------|
| "Jobs per status" | All jobs, per status |
| "Laws per status" | All laws, per status. Growth in the exhausted bars needs attention |
| "Failed jobs" | Turns yellow at 1 failed job and red at 5 |
| "Pending jobs" | Turns yellow at 10 and orange at 50. High and not dropping means the workers are not keeping up, or enrichment is at its hourly limit |
| "Avg job duration (24h)" | Mean run time of jobs completed in the last day. Yellow above a minute, red above five |
| "Completed jobs" | All completed jobs |
| "Jobs over time" | The job counts per status as a line, to see a backlog grow or shrink |
| "Avg job duration over time" | The mean duration as a line |

The numbers come from the harvester admin API and count the whole history, not only the selected period, except where the title says "24h".

## The daily summary

Every night between 00:00 and 01:00 Amsterdam time, Grafana posts "Dagelijkse Regelrecht Update" to the team's Mattermost channel. It is a summary, not an alarm: it is sent every day, also when nothing went wrong. It covers production only.

| Row | Meaning |
|-----|---------|
| "Geharvest" / "Verrijkt" | Laws currently harvested / enriched |
| "Mislukt harvesting (24u)" / "Mislukt verrijking (24u)" | Jobs that failed in the last 24 hours |
| "Totaal mislukt harvesting" / "Totaal mislukt verrijking" | All failed jobs |
| "Exhausted harvesting" / "Exhausted verrijking" | Laws the pipeline has given up on |
| "Openstaand" / "Afgerond" | Pending / completed jobs |

A day-on-day rise in the "Exhausted" rows is the signal to open "Wetten", filter on the exhausted status and work through the section above. A rise in the 24-hour failures without new exhausted laws usually means the pipeline is still retrying.
