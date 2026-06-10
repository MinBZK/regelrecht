#!/usr/bin/env python3
"""Per financieel-CV-wet: welke artikelen zijn relevant (= dragen machine_readable)?
Line-based parser (geen pyyaml-afhankelijkheid)."""
import glob, re, os

BASE = "corpus/regulation/nl/wet"
LAWS = {
    "ziektewet": "NRP — no-riskpolis",
    "wet_tegemoetkomingen_loondomein": "LKV / LIV",
    "participatiewet": "LKS",
    "wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten": "LDP — loondispensatie",
    "wet_werk_en_inkomen_naar_arbeidsvermogen": "JC / WPA (WIA, bron-wet)",
    "werkloosheidswet": "PP — proefplaatsing (WW, bron-wet)",
}

art_re   = re.compile(r"^  - number:\s*'?\"?([^'\"]+)'?\"?\s*$")
mr_re    = re.compile(r"^    machine_readable:\s*$")
out_re   = re.compile(r"^\s*- output:\s*([A-Za-z0-9_]+)")
lb_re    = re.compile(r"^\s*article:\s*'?\"?([^'\"]+)'?\"?\s*$")
reg_re   = re.compile(r"^\s*regulation:\s*([A-Za-z0-9_]+)")
text_re  = re.compile(r"^    text:\s*(.*)$")

def analyze(path):
    arts = []          # list of dicts for modeled articles
    cur = None
    in_mr = False
    pending_text = None
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            m = art_re.match(line)
            if m:
                if cur and cur["mr"]:
                    arts.append(cur)
                cur = {"num": m.group(1), "mr": False, "text": "",
                       "outputs": [], "lb": set(), "src": set()}
                in_mr = False
                pending_text = None
                continue
            if cur is None:
                continue
            tm = text_re.match(line)
            if tm and not cur["text"]:
                t = tm.group(1).strip().lstrip("|>-").strip()
                if t and t not in ("|", ">", "|-", ">-"):
                    cur["text"] = t
            if mr_re.match(line):
                cur["mr"] = True
                in_mr = True
            if cur["mr"]:
                om = out_re.match(line)
                if om:
                    o = om.group(1)
                    if o not in cur["outputs"]:
                        cur["outputs"].append(o)
                lm = lb_re.match(line)
                if lm and lm.group(1) != cur["num"]:
                    cur["lb"].add(lm.group(1))
                rm = reg_re.match(line)
                if rm:
                    cur["src"].add(rm.group(1))
    if cur and cur["mr"]:
        arts.append(cur)
    # total article count
    total = 0
    with open(path, encoding="utf-8") as f:
        for line in f:
            if art_re.match(line):
                total += 1
    return total, arts

for law, regeling in LAWS.items():
    files = sorted(glob.glob(f"{BASE}/{law}/*.yaml"))
    print("=" * 78)
    print(f"{law}   [{regeling}]")
    for path in files:
        ver = os.path.basename(path)
        total, arts = analyze(path)
        print(f"  └─ {ver}: {len(arts)} relevante (machine_readable) van {total} artikelen totaal")
        for a in arts:
            snippet = (a["text"][:70] + "…") if len(a["text"]) > 70 else a["text"]
            print(f"       • art. {a['num']}: {snippet}")
            if a["outputs"]:
                print(f"           produceert: {', '.join(a['outputs'])}")
            if a["lb"]:
                print(f"           legal_basis art.: {', '.join(sorted(a['lb']))}")
            if a["src"]:
                print(f"           cross-law bron: {', '.join(sorted(a['src']))}")
