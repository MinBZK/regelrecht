# Legal Review Findings — Wet studiefinanciering 2000

Date: 2026-04-09
Reviewer: reverse-validate skill

## Finding 1: heeft_diploma_binnen_termijn should be computed, not a parameter

**Article:** 12.30 lid 2c
**Severity:** High — scope violation

**Law text says:**
> "binnen de diplomatermijn hoger onderwijs of, indien hij geen
> studiefinanciering heeft aangevraagd, binnen tien jaar nadat hij zich
> voor het eerst heeft ingeschreven voor het hoger onderwijs, met goed
> gevolg een opleiding als bedoeld in artikel 5.7 heeft afgerond."

**Current model:** `heeft_diploma_binnen_termijn` is a boolean parameter
(external input). The legal chain Art. 12.30 → Art. 5.7 → Art. 5.2 is
not executed.

**Should be:** Computed via `source.output` from Art. 5.7, which in turn
references Art. 5.2 for the diplomatermijn. The law prescribes the
calculation; treating it as external input loses the legal reasoning chain.

**Status:** [ ] Open

---

## Finding 2: Alternative 10-year termijn for non-SF students is missing

**Article:** 12.30 lid 2c
**Severity:** High — missing legal path

**Law text says:**
> "of, **indien hij geen studiefinanciering heeft aangevraagd**, binnen
> **tien jaar** nadat hij zich voor het eerst heeft ingeschreven"

**Current model:** Only the diplomatermijn path exists. The alternative
10-year path for students who never applied for SF is not modeled.

**Should be:** An IF: if student had SF → check diplomatermijn (Art. 5.2),
if student had no SF → check 10-year limit from first enrolment date.

**Status:** [ ] Open

---

## Finding 3: Amount not capped at diplomatermijn

**Article:** 12.30 lid 3
**Severity:** Medium — missing constraint

**Law text says:**
> "tot een maximum van de periode, genoemd in artikel 5.2, eerste lid"

**Current model:** `bedrag_per_maand × maanden_studiefinanciering` without
any cap.

**Should be:** `bedrag_per_maand × MIN(maanden_studiefinanciering,
diplomatermijn_maanden)`

**Status:** [ ] Open

---

## Finding 4: Degree type check missing in Art. 5.7

**Article:** 5.7
**Severity:** Low — simplification

**Law text says:** Lists specific qualifying degree types:
- associate degree-opleiding (lid 1)
- hbo-bacheloropleiding (lid 3)
- hbo-masteropleiding (lid 3)
- wo-bacheloropleiding + wo-masteropleiding (lid 3)

**Current model:** Only checks `maanden_studie <= diplomatermijn_maanden`.
Does not verify the type of degree obtained.

**Should be:** Input for degree type, with check against the enumerated
list in Art. 5.7. This would make the "opleiding als bedoeld in artikel
5.7" reference in Art. 12.30 fully traceable.

**Status:** [ ] Open
