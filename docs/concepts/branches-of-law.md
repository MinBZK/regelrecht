# Branches of Law

RegelRecht makes Dutch law machine-readable and executable. Some branches of law lend themselves to this better than others. This page surveys the major branches, how well they map to rule-based execution, and what research questions they raise.

::: info Current Focus
RegelRecht currently focuses on **bestuursrecht** and **procesrecht**. The corpus contains regulations from administrative law, social security, healthcare, and tax law. Other branches are under research to understand where machine-readable law can and can't go.
:::

## Overview

| Branch | Dutch Name | Computability | Key Challenge |
|--------|-----------|---------------|---------------|
| Administrative Law | Bestuursrecht | High | Scale and cross-law references |
| Tax Law | Belastingrecht | High | Sheer volume and complexity |
| Social Security Law | Socialezekerheidsrecht | High | Overlaps with bestuursrecht |
| Electoral Law | Staatsrecht / Kiesrecht | High | Limited scope |
| Inheritance Law | Erfrecht | High | Family tree modeling |
| Criminal Law | Strafrecht | Medium | Judicial discretion within structured boundaries |
| Employment Law | Arbeidsrecht | Medium | Semi-mandatory provisions, CAO derogations |
| Immigration Law | Vreemdelingenrecht | Medium | Policy-driven, discretionary powers |
| Procedural Law | Procesrecht | Medium | Workflow-oriented rather than calculation-oriented |
| Health Law | Gezondheidsrecht | Medium | Clinical judgment as input |
| Environmental Law | Milieurecht | Medium | Location-dependent, layered derogations |
| Civil Liability | Aansprakelijkheidsrecht | Low | Open norms dominate |
| Constitutional Rights | Grondrechten | Low | Principle balancing, not rule application |
| Intellectual Property | Intellectueel eigendomsrecht | Low | Fact-intensive, subjective assessments |

## Analysis per Branch

### Bestuursrecht (Administrative Law)

Bestuursrecht is the natural home for machine-readable law. Government bodies apply structured rules to individual cases and produce formal decisions (*beschikkingen*). The AWB provides a uniform procedural framework; sector-specific laws define the substance.

Eligibility checks, benefit calculations, permit conditions: these follow deterministic if/then logic with defined inputs and outputs. The engine's cross-law reference mechanism maps directly to how administrative law works, where higher laws delegate to lower regulations and ministeriële regelingen fill in concrete values.

The corpus already contains the AWB, Participatiewet, Zorgtoeslag, WLZ, and Zorgverzekeringswet. The sheer number of laws, regulations, and policy rules that interact is what makes scaling hard.

### Belastingrecht (Tax Law)

Tax law is a giant calculation. Income tax, corporate tax, VAT, inheritance tax: inputs, brackets, rates, deductions, thresholds. The Wet IB 2001 alone is a massive decision tree referencing dozens of other laws for definitions and amounts.

In many ways this is the same paradigm as the toeslagen already in the corpus, just at larger scale. The cross-law `source` mechanism would get a workout: IB 2001 pulls definitions from the AWR, amounts from various regelingen, and conditions from sector-specific laws.

Volume is the obvious problem. Less obvious: anti-avoidance provisions like *fraus legis* are open-ended judicial doctrines that resist formalization entirely.

### Socialezekerheidsrecht (Social Security Law)

Social security overlaps heavily with bestuursrecht. Benefits are granted through beschikkingen and follow the same procedural framework. The WW, ZW, WIA, AOW, AKW, and Toeslagenwet all define structured calculations. AOW accrual: 2% per year of residency between age 15 and 67. WW duration: depends on employment history. Daily wage: defined reference periods.

This doesn't raise new modeling questions. It does provide a rich set of cross-law interactions that would stress-test reference resolution.

### Staatsrecht / Kiesrecht (Constitutional and Electoral Law)

The Kieswet is already in the corpus. Seat allocation algorithms, quorum requirements, voting procedures, electoral deadlines: all highly structured. The Gemeentewet and Provinciewet contain similar procedural rules for local government.

Beyond electoral mechanics, constitutional law is dominated by principles. Fundamental rights require proportionality balancing, which is judicial interpretation. The computable parts of this branch are limited in scope but well-defined.

### Erfrecht (Inheritance Law)

Intestate succession in BW Boek 4 is pure math. Fractions of the estate are distributed based on family tree structure and parentele. The *wettelijke verdeling*, *legitieme portie*, and *plaatsvervulling* all follow defined algorithms.

What makes this a worthwhile modeling exercise: the input is not a flat set of parameters but a graph of family relationships. The engine would need recursive or tree-based computation, which differs from the current linear decision-tree model.

### Strafrecht (Criminal Law)

Criminal law sits at the boundary between structured rules and judicial discretion. The *delictsomschrijvingen* in the Wetboek van Strafrecht are element-based: each offense has objective and subjective elements that must all be satisfied. This maps to checklists.

Several aspects are computable. Maximum and minimum sentences per offense have defined modifiers for attempt (2/3 of maximum), complicity (1/3 reduction), concurrence, and recidivism. Statutes of limitation are date arithmetic, categorized by offense severity. Defenses like *noodweer*, *overmacht*, and *ontoerekeningsvatbaarheid* have defined conditions.

Much of criminal law depends on judicial discretion, though. Sentencing happens within broad ranges. Concepts like *opzet* and *schuld* require human judgment about mental states. The engine can compute boundaries of what is legally possible, but not fill the space within those boundaries.

### Arbeidsrecht (Employment Law)

Employment law mixes structured rules with negotiable provisions. Dismissal law defines eight grounds (a through h in art. 7:669 BW) with structured conditions. The transition payment is a defined calculation. The *ketenregeling*, maximum three temporary contracts in three years, is counting logic. Minimum wage, holiday allowance, overtime: arithmetic.

Many employment law provisions are *semi-dwingend recht*, meaning collective labor agreements can override them. The engine would need to model statutory defaults plus the possibility of CAO-level overrides. This resembles the inversion-of-control pattern already explored in the corpus.

### Vreemdelingenrecht (Immigration Law)

Immigration law has structured decision trees. Each residence permit type has defined conditions and income requirements. The MVV requirement has a defined exemption list. Naturalization conditions are a checklist with defined exceptions. The Vreemdelingenwet 2000 is already in the corpus.

The law on paper is only part of the story. IND *werkinstructies* and policy rules matter as much as the statute, and discretionary powers like *schrijnendheid* allow deviation from the structured rules. Modeling this branch well means incorporating not just the wet but the entire policy layer underneath.

### Procesrecht (Procedural Law)

Procedural law is workflow-oriented rather than calculation-oriented. It defines sequences of steps, deadlines, competence rules, and conditions for procedural actions.

Deadline calculation is highly computable. Appeal periods, service requirements, and time limits follow strict rules from the AWB and the Wetboek van Burgerlijke Rechtsvordering. Court competence, which court handles which case type, is determined by subject matter, claim amount, and location: a structured decision tree.

The modeling question: procedural law is about sequencing and state transitions, not computing a single output value. This may need a different execution model, or it may fit as a series of connected regulations where each step's output feeds the next step's conditions.

### Gezondheidsrecht (Health Law)

Health law contains several procedurally structured laws. The Wvggz and Wzd define strict procedures for involuntary care with precise timelines and decision points. The Wet BIG defines registration requirements per healthcare profession.

The Wet forensische zorg is already in the corpus. The computable parts are procedural requirements and eligibility conditions. Clinical judgment that feeds into these decisions stays human.

### Milieurecht (Environmental Law)

The Omgevingswet, in effect since 2024, was explicitly designed with machine-readability in mind. The DSO/STOP-TPOD standards provide a data model for environmental plans and rules. The core question, "is a permit needed for activity X at location Y?", is a giant decision tree already implemented in the government's Omgevingsloket.

Complexity comes from layering: national rules, provincial rules, municipal rules, and location-specific plans all interact. Rules depend on location, making the input space enormous. This branch would push the engine toward spatial reasoning and plan-based rule resolution.

### Aansprakelijkheidsrecht (Civil Liability)

Liability law in BW Boek 6 is dominated by open norms: *redelijkheid en billijkheid*, *maatschappelijke betamelijkheid*, *toerekening naar redelijkheid*. There are structured elements (strict liability categories, limitation periods) but the core questions, whether conduct is wrongful and how to attribute damage, require case-by-case judicial assessment.

Useful mainly for modeling structured boundaries around otherwise discretionary decisions.

### Intellectueel Eigendomsrecht (Intellectual Property)

IP law has some computable elements. Copyright duration is 70 years *post mortem auctoris* with defined variations. Patent terms are fixed. Registration procedures have structured requirements.

Beyond that, IP disputes are fact-intensive and turn on subjective assessments of originality, similarity, and infringement. Limited algorithmic content.
