#!/usr/bin/env python3
"""Zet de vorige doorloop naast de nieuwe: links wat er stond, rechts wat er staat.

Leest beide gegenereerde pagina's, koppelt de panelen op wet + artikel, en toont
per rij de twee versies naast elkaar. Panelen die alleen rechts bestaan zijn
nieuw; panelen die alleen links bestaan zijn vervallen.

    python3 vergelijk.py <oud.html> <nieuw.html> <uit.html>
"""
import html
import re
import sys

OUD = sys.argv[1]
NIEUW = sys.argv[2]
UIT = sys.argv[3] if len(sys.argv) > 3 else 'vergelijking.html'

E = lambda s: html.escape(str(s), quote=True)


def ontleed(pad):
    """Haalt per persona de panelen uit een gegenereerde doorloop."""
    s = open(pad, encoding='utf8', errors='ignore').read()
    s = re.sub(r'<script[^>]*>.*?</script>', '', s, flags=re.S)
    uit = {}
    for sec in re.split(r'(?=<section class="persona">)', s)[1:]:
        m = re.search(r'<h2>([^<]*)</h2>', sec)
        naam = m.group(1).strip() if m else '?'
        panelen = []
        for st in re.split(r'(?=<div class="stap">)', sec)[1:]:
            wet = re.search(r'class="wet">([^<]*)', st)
            art = re.search(r'class="artikel">([^<]*)', st)
            chip = re.search(r'class="chip (c-[a-z-]+)">([^<]*)</span>', st)
            uitk = re.search(r'class="uitkomst">(.*?)</div>\s*(?:<div class="outputs">|<details)', st, re.S)
            ass = re.findall(r'<li>(.*?)</li>', st)
            panelen.append({
                'wet': wet.group(1).strip() if wet else '',
                'art': art.group(1).strip() if art else '',
                'chipklas': chip.group(1) if chip else 'c-ok',
                'chip': chip.group(2) if chip else '',
                'tekst': uitk.group(1).strip() if uitk else '',
                'asserts': ass,
            })
        if panelen:
            uit[naam] = panelen
    return uit


WET_ALIAS = {
    'wet wia': 'wia',
    'wet werk en inkomen naar arbeidsvermogen': 'wia',
    'wajong': 'wajong',
    'participatiewet': 'pwet',
    'wet financiering sociale verzekeringen': 'wfsv',
    'wet tegemoetkomingen loondomein': 'wtl',
    'ziektewet': 'zw',
    'werkloosheidswet': 'ww',
}
ART_ALIAS = {'10d': '10c'}


def sleutel(p):
    """Koppelt panelen tussen de versies.

    De vorige versie zette een toevoeging achter de wetnaam ("Participatiewet,
    loonkostensubsidie") en kortte de Wet WIA af; artikelnummers staan er soms
    samengevoegd ("art. 2.1 + 4.1", "art. 10c + 10d").
    """
    w = p['wet'].split(',')[0].strip().lower()
    w = WET_ALIAS.get(w, w)
    a = p['art'].lower().replace('art.', '').strip()
    a = re.split(r'\s*(?:\+|lid|en\b)\s*', a)[0].strip()
    a = ART_ALIAS.get(a, a)
    return (w, a)


def kernwaarden(asserts):
    """Uitkomstnaam -> waarde, zodat alleen echte verschillen tellen.

    De twee versies noemen niet dezelfde set asserties; vergelijken op de hele
    lijst zou elk paneel als gewijzigd markeren. Vergeleken wordt daarom alleen
    op de uitkomsten die in beide versies voorkomen.
    """
    uit = {}
    for a in asserts:
        t = re.sub(r'<[^>]+>', '', html.unescape(a)).strip()
        m = re.match(r'^([a-z0-9_]+)\s+is\s+(waar|onwaar)$', t)
        if m:
            uit[m.group(1)] = m.group(2)
            continue
        m = re.match(r'^([a-z0-9_]+)\s*=\s*(.+)$', t)
        if m:
            uit[m.group(1)] = m.group(2).split('(')[0].strip()
    return uit


def verschilt(op, np):
    """Waar als een gedeelde uitkomst van waarde verandert, of de chip anders is."""
    a, b = kernwaarden(op['asserts']), kernwaarden(np['asserts'])
    gedeeld = set(a) & set(b)
    anders = sorted(k for k in gedeeld if a[k] != b[k])
    return anders, op['chip'] != np['chip']


def kolom(p, kant):
    if p is None:
        woord = 'stond er nog niet' if kant == 'oud' else 'is vervallen'
        return f'<div class="kol leeg"><span>{woord}</span></div>'
    ass = ''.join(f'<li>{a}</li>' for a in p['asserts'][:8])
    meer = f'<li class="meer">nog {len(p["asserts"]) - 8}</li>' if len(p['asserts']) > 8 else ''
    return f'''<div class="kol">
  <div class="kop"><span class="wet">{E(p['wet'])}</span><span class="artikel">{E(p['art'])}</span><span class="chip {p['chipklas']}">{E(p['chip'])}</span></div>
  <div class="uitkomst">{p['tekst']}</div>
  <ul class="asserts">{ass}{meer}</ul>
</div>'''


CSS = '''
:root{--pap:#F4F6F8;--srf:#FFF;--snk:#EAEEF2;--ink:#141A21;--dim:#4A5563;--faint:#6E7A88;
--rule:#D8DEE6;--acc:#154273;--ok:#1F6141;--okw:#DDEDE3;--bad:#9B2C27;--badw:#F7E2E0;
--warn:#8A5300;--warnw:#F6E9D5;}
@media(prefers-color-scheme:dark){:root:not([data-theme=light]){--pap:#10151A;--srf:#191F26;
--snk:#212932;--ink:#E7ECF2;--dim:#A8B3C0;--faint:#808C9A;--rule:#2B333D;--acc:#7FB2DD;
--ok:#79C79B;--okw:#12291D;--bad:#EC8A84;--badw:#2F1917;--warn:#E0A557;--warnw:#2E2211;}}
*{box-sizing:border-box}
body{margin:0;background:var(--pap);color:var(--ink);font:16px/1.6 "Source Sans 3","Segoe UI",Arial,sans-serif}
.wrap{max-width:1400px;margin:0 auto;padding:0 24px 80px}
header.hoofd{padding:48px 0 24px;border-bottom:2px solid var(--acc);margin-bottom:32px}
.eyebrow{font:12px/1 ui-monospace,monospace;letter-spacing:.09em;text-transform:uppercase;color:var(--acc);margin:0 0 14px}
h1{font-size:clamp(28px,4vw,42px);line-height:1.1;margin:0 0 14px;letter-spacing:-.015em}
.dek{font-size:18px;color:var(--dim);max-width:70ch;margin:0 0 18px}
.stand{font:13px ui-monospace,monospace;color:var(--faint);margin:0}
h2{font-size:26px;margin:52px 0 6px;letter-spacing:-.01em}
.hint{color:var(--dim);margin:0 0 20px;max-width:70ch}
.balk{display:grid;grid-template-columns:1fr 1fr;gap:16px;position:sticky;top:0;background:var(--pap);
padding:10px 0;z-index:5;border-bottom:1px solid var(--rule)}
.balk div{font:12px ui-monospace,monospace;letter-spacing:.08em;text-transform:uppercase;color:var(--faint)}
.rij{display:grid;grid-template-columns:1fr 1fr;gap:16px;margin:16px 0;align-items:stretch}
.rij.gewijzigd{outline:2px solid var(--acc);outline-offset:6px;border-radius:2px}
.kol{background:var(--srf);border:1px solid var(--rule);padding:18px 20px;min-width:0}
.kol.leeg{display:flex;align-items:center;justify-content:center;background:transparent;
border-style:dashed;color:var(--faint);font:13px ui-monospace,monospace}
.kop{display:flex;flex-wrap:wrap;gap:8px 12px;align-items:baseline;margin-bottom:10px}
.wet{font-weight:700;font-size:15px}
.artikel{font:13px ui-monospace,monospace;color:var(--dim)}
.chip{font:11px ui-monospace,monospace;letter-spacing:.06em;text-transform:uppercase;
padding:2px 8px;border:1px solid currentColor}
.c-ok{color:var(--ok)}.c-nee{color:var(--bad)}.c-deels,.c-gewijzigd{color:var(--warn)}
.uitkomst{font-size:15px;color:var(--dim);margin-bottom:12px}
.uitkomst b{color:var(--ink)}
.asserts{margin:0;padding-left:1.1em;font:12.5px ui-monospace,monospace;color:var(--faint)}
.asserts li{margin-bottom:3px}
.asserts code{background:var(--snk);padding:0 3px}
.asserts b{color:var(--ink)}
.meer{list-style:none;margin-left:-1.1em;font-style:italic}
.merk{grid-column:1/-1;font:12px ui-monospace,monospace;letter-spacing:.07em;
text-transform:uppercase;color:var(--acc);margin-top:14px}
.samen{background:var(--srf);border:1px solid var(--rule);border-top:3px solid var(--acc);
padding:20px 22px;margin:28px 0}
.samen h3{margin:0 0 10px;font-size:17px}
.samen ul{margin:0;padding-left:1.15em;color:var(--dim)}
.samen li{margin-bottom:6px}
@media(max-width:900px){.rij,.balk{grid-template-columns:1fr}.kol.leeg{min-height:60px}}
'''


def bouw(oud, nieuw):
    secties = []
    tellers = {'gewijzigd': 0, 'nieuw': 0, 'vervallen': 0, 'gelijk': 0, 'benaming': 0}
    for naam, npanelen in nieuw.items():
        opanelen = oud.get(naam, [])
        okaart = {}
        for p in opanelen:
            okaart.setdefault(sleutel(p), []).append(p)
        rijen, gezien = [], set()
        for np in npanelen:
            k = sleutel(np)
            op = okaart[k].pop(0) if okaart.get(k) else None
            if op is not None:
                gezien.add(k)
            if op is None:
                merk, tellers['nieuw'] = 'nieuw in deze versie', tellers['nieuw'] + 1
            else:
                anders, chipanders = verschilt(op, np)
                if anders:
                    merk = f'uitkomst gewijzigd &middot; {E(", ".join(anders[:3]))}'
                    tellers['gewijzigd'] += 1
                elif chipanders:
                    merk = 'zelfde uitkomst, andere benaming'
                    tellers['benaming'] += 1
                else:
                    merk, tellers['gelijk'] = '', tellers['gelijk'] + 1
            klas = ' gewijzigd' if merk.startswith('uitkomst') or merk.startswith('nieuw') else ''
            mrk = f'<div class="merk">{merk}</div>' if merk else ''
            rijen.append(f'<div class="rij{klas}">{kolom(op,"oud")}{kolom(np,"nieuw")}{mrk}</div>')
        for k, rest in okaart.items():
            for op in rest:
                tellers['vervallen'] += 1
                rijen.append(f'<div class="rij gewijzigd">{kolom(op,"oud")}{kolom(None,"nieuw")}'
                             f'<div class="merk">vervallen</div></div>')
        secties.append(f'<h2>{E(naam)}</h2>'
                       f'<div class="balk"><div>25 augustus 2026</div><div>9 september 2026</div></div>'
                       + ''.join(rijen))

    sam = (f"<li><b>{tellers['gewijzigd']}</b> panelen waar de engine een andere uitkomst geeft</li>"
           f"<li><b>{tellers['nieuw']}</b> panelen nieuw &mdash; regelingen die er nog niet in zaten</li>"
           f"<li>{tellers['benaming']} panelen met dezelfde uitkomst, anders benoemd</li>"
           f"<li>{tellers['gelijk']} panelen ongewijzigd &middot; {tellers['vervallen']} vervallen</li>")

    return f'''<!doctype html>
<html lang="nl"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Doorloop, toen en nu</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Source+Sans+3:wght@400;600;700&display=swap">
<style>{CSS}</style></head><body><div class="wrap">
<header class="hoofd">
  <p class="eyebrow">Financieel CV &middot; doorloop &middot; vergelijking</p>
  <h1>Wat er veranderde aan de doorloop</h1>
  <p class="dek">Links de versie van 25 augustus, rechts die van 9 september. De rechterkant
  verwerkt de juristfeedback van 2 en 8 september en het antwoord dat SZW op 20 augustus in
  de editor gaf. Rijen met een rand eromheen zijn veranderd.</p>
  <p class="stand">Beide versies op peildatum 2026-07-01 &middot; de rechterkant is gegenereerd uit
  verse engine-traces</p>
</header>
<div class="samen"><h3>In één oogopslag</h3><ul>{sam}</ul></div>
{''.join(secties)}
</div></body></html>'''


if __name__ == '__main__':
    uit = bouw(ontleed(OUD), ontleed(NIEUW))
    open(UIT, 'w', encoding='utf8').write(uit)
    print(f'geschreven: {UIT} ({len(uit)} tekens)')
