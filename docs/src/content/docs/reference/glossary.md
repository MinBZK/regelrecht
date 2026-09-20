---
title: "Glossary"
description: "Dutch legal and technical terms used across RegelRecht, with English translations."
---

Dutch legal terminology used throughout RegelRecht, with English translations.

## Legal Hierarchy

These are the values of `regulatory_layer`, the schema field naming the kind of legal instrument a file is. The layer fixes which identifier the file must carry, and it ranks candidates when several regulations implement the same [open term](#regelrecht-specific-terms).

Note that *verordening* names four different things in this list. An EU-verordening is directly applicable across the Union and outranks national law; a gemeentelijke verordening is local. The shared word is not a shared concept.

| Value | Dutch | English | Description |
|-------|-------|---------|-------------|
| `GRONDWET` | Grondwet | Constitution | Supreme law of the Netherlands |
| `WET` | Wet | Act of Parliament | Formal legislation passed by the Staten-Generaal |
| `AMVB` | Algemene Maatregel van Bestuur (AMvB) | Order in Council | General administrative order by the Crown, on the basis of a wet |
| `KONINKLIJK_BESLUIT` | Koninklijk besluit (KB) | Royal decree | A decision of the Crown, used among other things to set entry into force |
| `MINISTERIELE_REGELING` | Ministeriële regeling | Ministerial regulation | Regulation issued by a minister under a delegation |
| `BELEIDSREGEL` | Beleidsregel | Policy rule | A rule on how a body uses a power it already has (Awb art. 1:3 lid 4). Binds the body, not the citizen directly |
| `EU_VERORDENING` | EU-verordening | EU Regulation | Directly applicable in every member state, without transposition |
| `EU_RICHTLIJN` | EU-richtlijn | EU Directive | Binding as to the result, transposed into national law by the member state |
| `VERDRAG` | Verdrag | Treaty | International agreement binding on the Netherlands |
| `UITVOERINGSBELEID` | Uitvoeringsbeleid | Implementation policy | Published practice of an executing body, below the level of a beleidsregel |
| `GEMEENTELIJKE_VERORDENING` | Gemeentelijke verordening | Municipal ordinance | Local regulation by a municipality |
| `PROVINCIALE_VERORDENING` | Provinciale verordening | Provincial ordinance | Regional regulation by a province |
| `WATERSCHAPS_VERORDENING` | Waterschapsverordening | Water authority ordinance | Regulation by a waterschap, the regional water authority |

## Administrative Law (Bestuursrecht)

| Dutch | English | Description |
|-------|---------|-------------|
| **Algemene wet bestuursrecht (Awb)** | General Administrative Law Act | Framework law for government decision-making |
| **Besluit** | Decision | A written decision of an administrative body constituting a public-law act (Awb art. 1:3 lid 1) |
| **Beschikking** | Individual decision | A besluit that is not of general scope (Awb art. 1:3 lid 2). The scope, not the number of people affected, is what distinguishes it |
| **Aanvraag** | Application | A request for a decision from a government body |
| **Bekendmaking** | Notification | Making a decision known to the addressee (Awb art. 3:41). The bezwaar period runs from this moment, not from the decision itself |
| **Bezwaar** | Objection | First recourse, filed with the body that decided |
| **Bezwaartermijn** | Objection period | Six weeks from bekendmaking (Awb art. 6:7 and 6:8) |
| **Beroep** | Appeal | Court appeal against a decision on bezwaar |
| **Belanghebbende** | Interested party | A person whose interest is directly affected by a besluit (Awb art. 1:2) |
| **Motivering** | Statement of reasons | The obligation to state the grounds a decision rests on (Awb art. 3:46) |

### Legal character

The values of `legal_character`, the schema field saying what kind of thing an output is.

| Value | Description |
|-------|-------------|
| `BESCHIKKING` | A besluit not of general scope (Awb art. 1:3 lid 2) |
| `BESLUIT_VAN_ALGEMENE_STREKKING` | A besluit that is of general scope, addressed to an open group |
| `RECHTSPOSITIE` | A legal position arising by operation of law, without a besluit and without an aanvraag (voting rights, majority, nationality) |
| `TOETS` | An assessment against a norm that is not itself a decision |
| `WAARDEBEPALING` | A determination of a value that other rules then use |
| `INFORMATIEF` | Informative output, carrying no legal consequence of its own |

### Decision type

The values of `decision_type`, saying what kind of decision an output is within its legal character.

| Value | Description |
|-------|-------------|
| `TOEKENNING` | Granting what was applied for |
| `AFWIJZING` | Refusing what was applied for |
| `GOEDKEURING` | Approval of an act by another body |
| `AANSLAG` | A tax assessment |
| `ALGEMEEN_VERBINDEND_VOORSCHRIFT` | A generally binding rule |
| `BELEIDSREGEL` | A policy rule on the use of an existing power (Awb art. 1:3 lid 4) |
| `VOORBEREIDINGSBESLUIT` | A preparatory decision taken ahead of a further one |
| `ANDERE_HANDELING` | Another act that is not one of the above |
| `GEEN_BESLUIT` | Not a decision at all; the article establishes a fact |

## Government Bodies

| Dutch | English | Description |
|-------|---------|-------------|
| **Bestuursorgaan** | Administrative body | Any government body making decisions |
| **Staten-Generaal** | Parliament | Dutch legislature (Eerste + Tweede Kamer) |
| **Tweede Kamer** | House of Representatives | Lower house of Parliament |
| **Eerste Kamer** | Senate | Upper house of Parliament |
| **Gemeente** | Municipality | Local government |
| **Belastingdienst** | Tax Authority | National tax administration |
| **SVB** | Social Insurance Bank | Executes social insurance laws |
| **UWV** | Employee Insurance Agency | Executes employee insurance laws |
| **Dienst Toeslagen** | Benefits Administration | Executes the income-dependent benefit schemes, zorgtoeslag among them |
| **Provincie** | Province | Regional government, one layer above the municipality |
| **Waterschap** | Water authority | Regional body for water management, with its own regulatory power |

## Legislative Process

| Dutch | English | Description |
|-------|---------|-------------|
| **Memorie van Toelichting (MvT)** | Explanatory Memorandum | Legislative intent document accompanying a bill |
| **Wetsvoorstel** | Bill | Proposed legislation before Parliament |
| **Staatsblad** | Official Gazette | Where national legislation is formally published |
| **Gemeenteblad** | Municipal Gazette | Where municipal ordinances are formally published |
| **Provincieblad** | Provincial Gazette | Where provincial ordinances are formally published |
| **Waterschapsblad** | Water Authority Gazette | Where waterschap ordinances are formally published |
| **Inwerkingtreding** | Entry into force | Date a law becomes effective |

## Temporal Concepts

| Dutch | English | Description |
|-------|---------|-------------|
| **Geldigheid** | Validity | Period during which a law is in force |
| **Peildatum** | Reference date | Date at which conditions are evaluated |
| **Terugwerkende kracht** | Retroactive effect | Law applies to situations before its enactment |

## RegelRecht-Specific Terms

| Term | Dutch | Description |
|------|-------|-------------|
| **Corpus** | Corpus | The git-versioned collection of machine-readable laws. A [federated corpus](/concepts/federated-corpus) spans more than one repository |
| **Open term** | Open norm | A value a higher law leaves to a lower regulation to fill. See [Inversion of Control](/concepts/inversion-of-control) |
| **Implements** | Gelet op | A lower regulation declaring which open terms of a higher law it fills. The schema field carries `gelet_op`, the citation the Dutch instrument itself opens with |
| **Cross-law reference** | Verwijzing | A law reading an output of another law through a `source` block. See [Cross-Law References](/concepts/cross-law-references) |
| **Marking** | Markering | A construct the format cannot yet express, flagged on the article. Called `untranslatables` before schema v0.7.0. See [Markings](/concepts/markings) |
| **Hook** | Haak | Logic that fires at a stage of a procedure rather than on a direct request. See [Hooks and Reactive Execution](/concepts/hooks-and-reactive-execution) |
| **Trace** | Spoor | The tree showing how each value in an execution was computed. See [Traceability](/concepts/traceability) |
| **Execution Receipt** | Uitvoeringsbewijs | The sealed record of one execution: engine, schema, law version and hash, so the result can be reproduced. See [Execution Provenance](/concepts/execution-provenance) |
| **Traject** | Traject | A working context in the editor, with its own members, roles and branch, optionally backed by its own repository |
| **Bevoegd gezag** | Bevoegd gezag | The body competent to take a decision under a given article. See [Competent Authority](/concepts/competent-authority) |
| **Absent vs. unknown** | Afwezig versus onbekend | A value that does not exist, against a value nobody has. The engine keeps them apart rather than treating both as empty |
| **Bucket A / Bucket B** | - | The two BDD suites: law validation against the real corpus, and engine conformance against synthetic laws. See [Testing](/guide/testing) |

## Abbreviations

| Term | Stands for | Description |
|------|------------|-------------|
| **Awb** | Algemene wet bestuursrecht | The general administrative law act |
| **Awir** | Algemene wet inkomensafhankelijke regelingen | The framework act for income-dependent benefit schemes |
| **BWB** | Basis Wettelijke Regelgeving | The national register of legislation, and the source the harvester reads |
| **CVDR** | Centrale Voorziening Decentrale Regelgeving | The register of decentralized (municipal, provincial, water authority) regulation |
| **MvT** | Memorie van Toelichting | The explanatory memorandum accompanying a bill |
| **FSC** | Federated Service Connectivity | The standard for service-to-service connections between government organizations |
| **JAS** | Juridisch Analyseschema | A legal analysis schema used in wetsanalyse |
