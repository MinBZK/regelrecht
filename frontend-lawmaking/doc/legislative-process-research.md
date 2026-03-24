# Dutch Legislative Process — Research for Advanced Visualization

Research for issue #14, mapping the Dutch law-making process to Git/CI/CD concepts.

## Sources

### Academic
- **Wim Voermans** — LEDA system (knowledge-based legislative drafting), "Sturen in de mist" (1995), "Wetgevingsprocessen in transitie" (2012)
- **Mariette Lokin** — "Wendbaar wetgeven" (2018), LegOps model, Wetsanalyse method, RegelSpraak CNL
- Voermans & Lokin did not produce explicit BPMN diagrams; Voermans used frame-based knowledge representation, Lokin used agile/iterative process descriptions

### Official
- [Draaiboek voor de regelgeving (KCBR)](https://www.kcbr.nl/beleid-en-regelgeving-ontwikkelen/draaiboek-voor-de-regelgeving) — items nr. 4-110
- [Overzicht procedure wet (KCBR)](https://www.kcbr.nl/beleid-en-regelgeving-ontwikkelen/draaiboek-voor-de-regelgeving/bijlagen/bijlagen-overzichten-schemas-en-handleidingen/overzicht-procedure-wet)
- [Hoe komt een wet tot stand (Rijksoverheid)](https://www.rijksoverheid.nl/onderwerpen/wetgeving/hoe-komt-een-wet-tot-stand)
- [Wetgevingsprocedure (Eerste Kamer)](https://www.eerstekamer.nl/wetgevingsprocedure)
- [Aanwijzingen voor de regelgeving (KCBR)](https://www.kcbr.nl/ontwikkelen-beleid-en-regelgeving/aanwijzingen-voor-de-regelgeving)

---

## Full Legislative Lifecycle

### Phase A: Ministry Internal Preparation

| Step | Description | Git Analogy |
|------|-------------|-------------|
| A1. Policy idea | Coalition agreement, EU directive, court ruling, societal problem | Issue / feature request |
| A2. Internal drafting | Wetgevingsjurist + beleidsmedewerker draft bill + MvT | Local commits on feature branch |
| A3. Quality review | Directie Wetgeving/JZ reviews against Aanwijzingen voor de regelgeving | Internal code review |
| A4. Sign-off chain | Wetgevingsjurist → section head → director → DG → SG → minister | Required approvals before PR |

**Roles:**
- **Beleidsmedewerker** (policy officer) — defines what the law should achieve
- **Wetgevingsjurist** (legislative counsel) — translates policy into legal text
- **Directeur Wetgeving / Hoofd JZ** — supervises legislative function
- **JenV wetgevingstoets** — Ministry of Justice reviews other ministries' bills on rule-of-law quality

### Phase B: Interdepartmental Preparation

| Step | Description | Git Analogy |
|------|-------------|-------------|
| B1. Interdepartmental consultation | Circulated to all relevant ministries (IOWJZ, IHW, IWB) | Cross-team code review |
| B2. Mandatory quality tests | See below | CI pipeline checks |
| B3. Ambtelijk voorportaal | Senior civil servants resolve disagreements | Architecture review board |

**Mandatory CI checks:**
- Uitvoerbaarheid en handhaafbaarheid (U&H) — feasibility test by UWV, Belastingdienst, SVB
- Regeldruktoets — regulatory burden (Adviescollege Toetsing Regeldruk / ATR)
- Financiele toets — Ministry of Finance
- Gevolgen voor de rechtspraak — Raad voor de Rechtspraak
- Privacy/DPIA, environmental impact, etc.

### Phase C: External Consultation

| Step | Description | Git Analogy |
|------|-------------|-------------|
| C1. Internetconsultatie | 4+ weeks public comment on internetconsultatie.nl | Public RFC / open comment period |
| C2. Advisory bodies | SER, Raad voor de Rechtspraak, Autoriteit Persoonsgegevens | Mandatory domain-expert reviewers |

### Phase D: Cabinet Decision

| Step | Description | Git Analogy |
|------|-------------|-------------|
| D1. Onderraad | Relevant ministerial sub-council discusses | Team lead review |
| D2. Ministerraad | Full cabinet approves (every Friday) | Project lead merge approval |

### Phase E: Raad van State

| Step | Description | Git Analogy |
|------|-------------|-------------|
| E1. Adviesaanvraag | Bill sent to RvS via the King | Request external audit |
| E2. Advisory opinion | Review on constitutional, legal, and drafting quality; dictum | Senior architect review |
| E3. Nader rapport | Government responds, may modify bill | Address review feedback, push commits |

### Phase F: Tweede Kamer

| Step | Description | Git Analogy |
|------|-------------|-------------|
| F1. Indiening | Koninklijke Boodschap sends bill to TK | `git push` + open PR |
| F2. Verslag | Committee written questions | PR review comments |
| F3. Nota n.a.v. verslag | Government answers (can repeat) | Author responds to review |
| F4. Nota van wijziging | Government modifies own bill | Author pushes new commits |
| F5. Wetgevingsoverleg | Article-by-article committee discussion | Line-by-line code review |
| F6. Plenair debat | Full chamber debate | PR discussion in team meeting |
| F7. Amendementen | MPs propose changes | Reviewer-submitted patches |
| F8. Subamendementen | Amendments to amendments (1 level deep) | PR against a PR branch |
| F9. Stemmingen | Vote per article, then whole bill | Merge approval process |

**Amendment mechanics:**
- Voting order: subamendementen first, then amendementen, "verste strekking" (most far-reaching) first
- Government can "overnemen" (adopt) = cherry-pick, no vote needed
- Government can "ontraden" (advise against)
- Stemmingslijst = merge queue with conflict detection
- Conflicting amendments: accepting one may make another "vervallen" (moot)

### Phase G: Eerste Kamer

| Step | Description | Git Analogy |
|------|-------------|-------------|
| G1. Voorlopig verslag | Committee written questions | Final reviewer comments |
| G2. Memorie van antwoord | Government responds | Author responds |
| G3. Plenair debat | Senate debate | Final review meeting |
| G4. Stemming | Accept or reject only (NO amendments) | Protected branch: approve/reject |

**Novelle mechanism:**
- If EK has concerns: government submits a new separate bill (novelle) that amends the original
- Novelle goes through full TK pipeline first
- Both bills treated simultaneously in EK, both must pass
- Git: a dependent fix-PR that must pass full CI before the original can merge

### Phase H: Publication & Entry into Force

| Step | Description | Git Analogy |
|------|-------------|-------------|
| H1. Bekrachtiging | King signs + minister countersigns | Merge to main |
| H2. Staatsblad | Publication in Official Gazette | Release tag |
| H3. Inwerkingtreding | Entry into force (fixed date, KB, or day after pub) | Deploy to production |

**Inwerkingtreding variants:**
- Fixed date in law itself
- By separate Koninklijk Besluit (date determined later)
- Different articles on different dates = phased rollout
- Vaste verandermomenten: Jan 1 and Jul 1 preferred

---

## Special Procedures

| Type | Description | Git Analogy |
|------|-------------|-------------|
| Initiatiefwetsvoorstel | Private member's bill (any TK member) | External contributor PR |
| AMvB | General administrative measure (delegated, no parliament unless voorhang) | Config change under delegated authority |
| Voorhangprocedure | Parliament gets review period on AMvB before RvS | Review gate on implementation |
| Spoedwetgeving | Emergency legislation, same steps but accelerated | Hotfix with expedited review |
| Noodwet | Emergency decree, rules before parliament votes | Push to production, retroactive PR |

---

## Cross-Law Relationships

| Type | Description | Git Analogy |
|------|-------------|-------------|
| Wijzigingswet | Modifies an existing law | PR that modifies existing code |
| Invoeringswet | Handles transition from old to new law | Migration script |
| Aanpassingswet | Updates multiple laws to align with new major law | Dependency update PR |
| Reparatiewet | Fixes technical errors across multiple laws | Batch bugfix |
| Verzamelwet | Bundles small amendments to multiple laws | Omnibus commit |
| Samenloopbepaling | Pre-programmed conflict resolution when two bills modify same law | Pre-written rebase strategy: "if A merges first, apply X; if B first, apply Y" |

---

## Git Branch Model (Advanced)

```
main (Corpus Juris — Staatsblad published law)
 │
 ├── feature/wetsvoorstel-12345  (the bill)
 │    │
 │    ├── [commits: ministry drafts, internal review]
 │    ├── [commit: interdepartmental feedback]
 │    ├── [commit: internetconsultatie response]
 │    ├── [commit: nader rapport response to RvS]
 │    ├── [commit: nota van wijziging 1]
 │    ├── [commit: nota van wijziging 2]
 │    │
 │    ├── patch/amendement-nr-7  (MP's amendment)
 │    │    │
 │    │    └── patch/subamendement-nr-12  (amendment to amendment)
 │    │        [voted first; if accepted, modifies parent]
 │    │
 │    ├── patch/amendement-nr-8  (may conflict with nr-7)
 │    │    [voting order resolves: verste strekking first]
 │    │
 │    ├── patch/amendement-nr-9  (adopted by government)
 │    │    [cherry-picked directly, no vote]
 │    │
 │    │  [== stemmingen: merge queue processes patches ==]
 │    │  [== gewijzigd voorstel van wet ==]
 │    │  [== Eerste Kamer: accept/reject gate ==]
 │    │
 │    └── feature/novelle-12346  (if EK has concerns)
 │         ├── [goes through full TK pipeline]
 │         [merged simultaneously with parent]
 │
 ├── feature/wetsvoorstel-12400  (concurrent bill modifying same law)
 │    └── [includes samenloopbepaling for conflict with 12345]
 │
 ├── config/amvb-xyz  (delegated legislation)
 │    └── [voorhang review gate if required]
 │
 └── hotfix/spoedwet-abc  (emergency legislation)
      └── [same steps, accelerated timeline]
```

---

## Key Insights for Visualization

1. **The ministry phase is a "monorepo" with internal swimlanes** — policy officers, legislative counsel, and management all work within the same organizational boundary but with distinct roles and handoffs.

2. **The stemmingslijst is THE merge queue** — Bureau Wetgeving constructs the voting order to prevent contradictions, exactly like a CI-enforced merge queue with conflict detection.

3. **Samenloopbepalingen are unique** — proactive, pre-programmed merge conflict resolution that has no direct equivalent in Git. This is a legislative invention worth highlighting.

4. **The novelle is an elegant workaround** — a dependent fix-PR that must pass the full pipeline before the original can merge, because the final reviewer (Eerste Kamer) cannot modify.

5. **"Overnemen" is cherry-pick** — the government adopting an amendment bypasses the vote entirely.

6. **Rebasing happens via nota van wijziging** — when underlying law changes during drafting, the government updates references by filing a nota van wijziging.
