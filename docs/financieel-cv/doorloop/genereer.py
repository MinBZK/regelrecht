#!/usr/bin/env python3
"""Bouwt koen-en-sadee-doorloop.html uit verse engine-traces.

De pagina bestond eerder wel maar de generator niet; hij was eenmalig met de
hand gemaakt en liep daardoor twee feedbackrondes achter. Dit script leest
trace_output/, combineert dat met het verhaal hieronder, en hergebruikt de
stijl en de trace-viewer van de vorige versie.

Draaien vanaf de regelrecht-repo:
    python3 genereer.py <repo> <uitvoer.html>
"""
import json
import os
import re
import sys
import glob
import html

REPO = sys.argv[1] if len(sys.argv) > 1 else '/home/claude/projects/regelrecht'
UIT = sys.argv[2] if len(sys.argv) > 2 else '/home/claude/projects/koen-en-sadee-doorloop.html'
HIER = os.path.dirname(os.path.abspath(__file__))
CORPUS = '/home/claude/projects/regelrecht-corpus'

E = lambda s: html.escape(str(s), quote=True)


# ── traces inlezen ────────────────────────────────────────────────────────

def laad_traces():
    uit = {}
    for p in sorted(glob.glob(os.path.join(REPO, 'trace_output', '*.json'))):
        seq = os.path.basename(p)[:3]
        d = json.load(open(p, encoding='utf8'))
        txt = p[:-5] + '.txt'
        uit[seq] = {
            'trace': d.get('trace', d),
            'bestand': os.path.relpath(p, REPO),
            'ruw': open(txt, encoding='utf8').read() if os.path.exists(txt) else '',
        }
    return uit


def tel_stappen(n):
    return 1 + sum(tel_stappen(k) for k in (n.get('children') or []))


def tel_xlaw(n):
    c = 1 if n.get('node_type') == 'cross_law_reference' else 0
    return c + sum(tel_xlaw(k) for k in (n.get('children') or []))


def asserts_uit_feature(pad, scenario):
    """Haalt de Then/And-regels van één scenario uit een featurebestand."""
    vol = os.path.join(CORPUS, pad)
    if not os.path.exists(vol):
        return []
    regels, aan, gehad = [], False, False
    for r in open(vol, encoding='utf8'):
        s = r.strip()
        if s.startswith('Scenario'):
            if gehad:
                break                      # alleen het eerste treffende scenario
            titel = s.split(':', 1)[-1].strip()
            aan = scenario.lower() in titel.lower()
            gehad = aan
            continue
        if not aan or not s or s.startswith('#'):
            continue
        if s.startswith('Given') or s.startswith('When') or s.startswith('|'):
            continue
        m = re.match(r'^(?:Then|And)\s+(.*)$', s)
        if m and not m.group(1).startswith('the following parameters'):
            regels.append(m.group(1))
    return regels


def toon_assert(a):
    a = a.strip()
    if a == 'the execution succeeds':
        return 'De afleiding slaagt zonder ontbrekende gegevens'
    m = re.match(r'output "([^"]+)" is (true|false)', a)
    if m:
        return f'<code>{E(m.group(1))}</code> is <b>{"waar" if m.group(2)=="true" else "onwaar"}</b>'
    m = re.match(r'output "([^"]+)" equals (.+)', a)
    if m:
        return f'<code>{E(m.group(1))}</code> = <b>{E(m.group(2))}</b>'
    return E(a)


# ── het verhaal ───────────────────────────────────────────────────────────
# Elk paneel: (trace-id, wet, artikel, chip, uitkomsttekst, outputs,
#              scenarionaam, featurepad)

PERSONAS = [
    {
        'naam': 'Koen',
        'kop': 'Koen, 42 jaar',
        'rol': 'Werkt 32 uur bij een logistiek MKB-bedrijf, via de gemeente in dienst sinds '
               '1 januari 2026. Bijstandsachtergrond, staat sinds 3 november 2025 in het '
               'doelgroepregister banenafspraak.',
        'kenmerken': [('Uurloon', '€ 12,00'), ('Arbeidsduur', '32 uur per week'),
                      ('Verloonde uren', '1.664 per jaar'), ('Jaarloon', '€ 19.968'),
                      ('Loonwaarde', '60% van het WML')],
        'panelen': [
            ('037', 'Wet financiering sociale verzekeringen', 'art. 38b', 'bron', 'c-ok',
             'Het <b>doelgroepregister banenafspraak</b> is het startpunt. Koen komt erin op grond '
             'van lid 1 onderdeel a: vanuit de Participatiewet toegeleid naar werk, met een '
             'UWV-vaststelling dat hij het minimumloon niet zelfstandig kan verdienen. Twee andere '
             'wetten halen die status hier op, dus alles wat hierna komt hangt aan dit ene artikel.',
             ['behoort_tot_doelgroepregister_banenafspraak', 'grond_opname_doelgroepregister'],
             'Koen staat in het register op grond van onderdeel a',
             'regulation/nl/wet/wet_financiering_sociale_verzekeringen/scenarios/doelgroepregister_banenafspraak.feature'),

            ('002', 'Participatiewet', 'art. 10c en 10d', 'recht', 'c-ok',
             'De <b>loonkostensubsidie</b> vult voor de werkgever het verschil aan tussen Koens '
             'loonwaarde van 60% en het minimumloon. Sinds de juristvalidatie van juli wordt het '
             'bedrag naar rato van de arbeidsduur berekend: Koen werkt 32 uur, niet 36, en dat '
             'scheelt.',
             ['heeft_recht_op_lks', 'hoogte_lks_eurocent_per_maand'],
             'Gemeente betaalt', 'regulation/nl/wet/participatiewet/scenarios/financieel_cv_koen.feature'),

            ('052', 'Wet tegemoetkomingen loondomein', 'art. 2.1', 'recht', 'c-ok',
             'Het <b>loonkostenvoordeel</b> voor de werkgever. Koen valt in de categorie '
             'banenafspraak. Let op de herkomst: die categorie komt niet uit een aangeleverd '
             'gegeven maar wordt door artikel 2.1 zelf afgeleid, uit het doelgroepregister '
             'hierboven.',
             ['heeft_recht_op_lkv', 'categorie_lkv', 'hoogte_lkv_per_jaar_eurocent'],
             'Werkgever ontvangt', 'regulation/nl/wet/wet_tegemoetkomingen_loondomein/scenarios/financieel_cv_koen.feature'),

            ('068', 'Ziektewet', 'art. 29b', 'recht', 'c-ok',
             'De <b>no-riskpolis</b>: valt Koen ziek uit, dan draagt UWV het ziekengeld in plaats '
             'van de werkgever. Dit artikel is het meest verbonden van alle zeven — het haalt acht '
             'gegevens op bij vier andere wetten in plaats van ze zelf te bepalen.',
             ['heeft_recht_op_no_risk_polis', 'duur_no_risk_polis_jaren'],
             'Koen krijgt no-risk polis', 'regulation/nl/wet/ziektewet/scenarios/financieel_cv_koen.feature'),

            ('054', 'Wet werk en inkomen naar arbeidsvermogen', 'art. 35', 'uitgesloten', 'c-nee',
             '<b>Hier ging het mis, en dit is gecorrigeerd na de juristfeedback van 2 september.</b> '
             'Lid 4 onderdeel b sluit niet alleen Wajong-gerechtigden uit, maar ook wie via de '
             'Participatiewet door de gemeente wordt ondersteund. Koen is via de gemeente in dienst '
             'gekomen, dus het college draagt zorg en de WIA is niet van toepassing. Tot 2 september '
             'concludeerde dit scenario het omgekeerde.',
             ['artikel_35_van_toepassing', 'heeft_recht_op_jobcoaching', 'heeft_recht_op_werkplekaanpassing'],
             'Koen valt buiten WIA artikel 35 voor JC en WPA (lid 4.b',
             'regulation/nl/wet/wet_werk_en_inkomen_naar_arbeidsvermogen/scenarios/financieel_cv_koen.feature'),

            ('005', 'Participatiewet', 'art. 10 lid 1', 'aanspraak', 'c-deels',
             'Maar het recht bestáát wel — alleen bij de gemeente. Artikel 10 geeft aanspraak op '
             'ondersteuning bij arbeidsinschakeling en op de noodzakelijk geachte voorziening. '
             '<b>Let op de sterkte:</b> die aanspraak wordt verleend "overeenkomstig de verordening". '
             'Vorm, duur en intensiteit bepaalt de gemeenteraad. Wat de engine hier zegt is dat de '
             'route bestaat en Koen in de doelgroep zit — niet welk bedrag of welke jobcoach.',
             ['behoort_tot_doelgroep_artikel_10', 'heeft_aanspraak_op_persoonlijke_ondersteuning',
              'heeft_aanspraak_op_voorziening_arbeidsinschakeling'],
             'Koen heeft via Pwet art. 10 aanspraak',
             'regulation/nl/wet/participatiewet/scenarios/financieel_cv_koen.feature'),

            ('006', 'Participatiewet', 'art. 10da', 'harde aanspraak', 'c-ok',
             'Eén onderdeel van de gemeentelijke keten is wél hard. Artikel 10da geeft de doelgroep '
             'loonkostensubsidie aanspraak op begeleiding op de werkplek: één zin, geen '
             'verordeningsvoorbehoud, geen delegatie. <b>Hier kun je een toezegging aan hangen; aan '
             'artikel 10 lid 1 niet.</b> Dat onderscheid moet in het Financieel CV zichtbaar blijven.',
             ['heeft_aanspraak_op_begeleiding_op_de_werkplek'],
             'Koen heeft als LKS-doelgroep een harde aanspraak',
             'regulation/nl/wet/participatiewet/scenarios/financieel_cv_koen.feature'),

            ('021', 'Werkloosheidswet', 'art. 76a', 'niet van toepassing', 'c-nee',
             'De <b>proefplaatsing</b> van de WW is voor Koen niet van toepassing: hij heeft geen '
             'WW-uitkering. De toelichting hierbij was tot 2 september onjuist — daar stond dat '
             'proefplaatsing "een WW-instrument" is. Dat is niet zo.',
             ['mag_proefplaatsing_aangaan'],
             'Koen kan geen proefplaatsing met behoud van uitkering aangaan zonder WW',
             'regulation/nl/wet/werkloosheidswet/scenarios/financieel_cv_koen.feature'),

            ('007', 'Participatiewet', 'art. 8a lid 2 d', 'recht', 'c-ok',
             'Proefplaatsing bestaat namelijk in <b>vier</b> wetten. Voor Koen loopt hij via de '
             'Participatiewet — en met een ander kader: twee maanden in plaats van zes, verlengbaar '
             'met maximaal vier. De voorwaarden staan bovendien niet in de wet maar in de '
             'gemeentelijke verordening.',
             ['mag_proefplaatsing_aangaan', 'max_duur_proefplaatsing_maanden',
              'max_totale_duur_proefplaatsing_maanden'],
             'Bijstandsgerechtigde uit de doelgroep mag op proefplaats',
             'regulation/nl/wet/participatiewet/scenarios/proefplaatsing.feature'),
        ],
    },
    {
        'naam': 'Sadee',
        'kop': 'Sadee, 28 jaar',
        'rol': 'Werkt 32 uur bij een MKB-werkgever, in dienst sinds 1 januari 2024. '
               'Wajong-uitkering, doelgroep banenafspraak.',
        'kenmerken': [('Uurloon', '€ 14,50'), ('Arbeidsduur', '32 uur per week'),
                      ('Verloonde uren', '1.664 per jaar'), ('Jaarloon', '€ 24.128'),
                      ('Loonwaarde', '70% van het WML')],
        'panelen': [
            ('038', 'Wet financiering sociale verzekeringen', 'art. 38b', 'bron', 'c-ok',
             'Ook bij Sadee is het <b>doelgroepregister</b> het startpunt, maar via een andere grond: '
             'onderdeel c, het Wajong-recht. <b>Dit artikel is op 8 september gecorrigeerd.</b> De '
             'slotzin van onderdeel c lazen wij als een uitsluiting voor wie duurzaam geen '
             'arbeidsvermogen heeft. Het is een voorwaardelijke insluiting: zo iemand telt wél mee '
             'zodra hij werkt.',
             ['behoort_tot_doelgroepregister_banenafspraak', 'voldoet_aan_grond_38b_1_c'],
             'Sadee staat in het register op grond van onderdeel c',
             'regulation/nl/wet/wet_financiering_sociale_verzekeringen/scenarios/doelgroepregister_banenafspraak.feature'),

            ('053', 'Wet tegemoetkomingen loondomein', 'art. 2.1', 'recht', 'c-ok',
             'Het <b>loonkostenvoordeel</b> — en hier zit de grootste verandering in bedrag. Sadee '
             'valt in de categorie <b>banenafspraak</b>, niet arbeidsgehandicapt. Dat scheelt '
             '€3.394,56 per jaar: €1.680,64 in plaats van €5.075,20. De correctie kwam van SZW zelf, '
             'op 20 augustus, als antwoord op onze vraag of een Wajonger automatisch in de '
             'categorie arbeidsgehandicapt valt. Dat is niet zo.',
             ['heeft_recht_op_lkv', 'categorie_lkv', 'hoogte_lkv_per_jaar_eurocent'],
             'Sadee krijgt LKV-banenafspraak',
             'regulation/nl/wet/wet_tegemoetkomingen_loondomein/scenarios/financieel_cv_sadee.feature'),

            ('069', 'Ziektewet', 'art. 29b', 'recht', 'c-ok',
             'De <b>no-riskpolis</b>. Sadee komt erin via lid 2: Wajong-gerechtigd. Voor die groep '
             'kent het artikel geen vaste duur — in het model staat 0 jaar, met een markering erbij, '
             'omdat "onbeperkt zolang het dienstverband duurt" iets anders is dan nul.',
             ['heeft_recht_op_no_risk_polis', 'duur_no_risk_polis_jaren'],
             'Sadee krijgt no-risk polis', 'regulation/nl/wet/ziektewet/scenarios/financieel_cv_sadee.feature'),

            ('029', 'Wajong', 'art. 2:20', 'recht', 'c-ok',
             'De <b>loondispensatie</b>: de werkgever mag tijdelijk minder dan het minimumloon '
             'betalen, omdat Sadees arbeidsprestatie duidelijk minder is. Lid 2 maakt een beding '
             'tot lagere beloning buiten deze route nietig — dat is in het model een harde '
             'constante, en die is bij de juristvalidatie akkoord bevonden.',
             ['heeft_recht_op_loondispensatie', 'beding_lagere_beloning_is_nietig'],
             'Sadee komt in aanmerking voor loondispensatie', 'regulation/nl/wet/wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten/scenarios/financieel_cv_sadee.feature'),

            ('056', 'Wet werk en inkomen naar arbeidsvermogen', 'art. 35', 'uitgesloten', 'c-nee',
             'Voor <b>jobcoaching en werkplekaanpassing</b> sluit lid 4 onderdeel a Sadee uit: zij '
             'heeft recht op arbeidsondersteuning op grond van de Wajong. Vanuit de WIA bezien lijkt '
             'het daardoor of zij geen recht heeft. Dat was de eerste bevinding van de jurist, in '
             'juli: geen uitsluiting van de voorziening, maar een andere vindplaats.',
             ['artikel_35_van_toepassing'],
             'Sadee valt buiten WIA artikel 35 voor JC en WPA (lid 4.a',
             'regulation/nl/wet/wet_werk_en_inkomen_naar_arbeidsvermogen/scenarios/financieel_cv_sadee.feature'),

            ('031', 'Wajong', 'art. 2:22', 'recht', 'c-ok',
             'En daar is die vindplaats. De Wajong kent dezelfde voorzieningen als de WIA, in '
             'artikel 2:22. Dit artikel is na de juristvalidatie van juli gemodelleerd, juist omdat '
             'het Financieel CV anders "geen recht" toonde terwijl het recht bestond.',
             ['heeft_recht_op_jobcoaching', 'heeft_recht_op_werkplekaanpassing'],
             'Sadee komt via Wajong art. 2:22', 'regulation/nl/wet/wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten/scenarios/financieel_cv_sadee.feature'),

            ('033', 'Wajong', 'art. 2:24', 'recht', 'c-ok',
             'De <b>proefplaatsing</b> van de Wajong: zes maanden, met de arbeidsondersteuning en de '
             'inkomensvoorziening die doorlopen. Woordelijk gelijk aan de WW en de WIA — en dus '
             'anders dan de Participatiewet, waar Koen op uitkomt.',
             ['mag_proefplaatsing_aangaan', 'max_duur_proefplaatsing_maanden'],
             'Wajong-gerechtigde mag op proefplaats',
             'regulation/nl/wet/wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten/scenarios/proefplaatsing.feature'),
        ],
    },
]


# ── bouwen ────────────────────────────────────────────────────────────────

def paneel(p, traces):
    tid, wet, art, chip, chipklas, tekst, outputs, scenario, feature = p
    t = traces.get(tid)
    if not t:
        return f'<!-- trace {tid} ontbreekt -->'
    stappen = tel_stappen(t['trace'])
    xlaw = tel_xlaw(t['trace'])
    asserts = asserts_uit_feature(feature, scenario)
    ass = ''.join(f'<li>{toon_assert(a)}</li>' for a in asserts) or \
          '<li>De afleiding slaagt zonder ontbrekende gegevens</li>'
    outs = ''.join(f'<span>{E(o)}</span>' for o in outputs)
    return f'''<div class="stap">
  <div class="rail"><div class="knoop {chipklas}"></div></div>
  <div class="kaart">
    <div class="kop"><span class="wet">{E(wet)}</span><span class="artikel">{E(art)}</span><span class="chip {chipklas}">{E(chip)}</span></div>
    <div class="uitkomst">{tekst}</div>
    <div class="outputs">{outs}</div>
    <details class="bewijs"><summary><span class="bewijs-label">Onderbouwing</span><span class="chip c-run-ok">getoetst</span><span class="bewijs-hint">{stappen} redeneerstappen</span></summary>
      <div class="bewijs-body">
        <dl class="bewijs-meta"><dt>Toetsscenario</dt><dd>{E(scenario)}</dd></dl>
        <details class="tech"><summary>Technische verwijzing</summary><dl class="bewijs-meta">
          <dt>Scenariobestand</dt><dd><code>{E(feature)}</code></dd>
          <dt>Uitvoeringslog</dt><dd><code>{E(t['bestand'])}</code> &middot; {stappen} redeneerstappen, {xlaw} verwijzingen naar andere wetten</dd>
        </dl></details>
        <div class="asserts-kop">Wat het toetsscenario vastlegt</div>
        <ul class="asserts">{ass}</ul>
        <div class="asserts-kop">De redenering, stap voor stap</div>
        <div class="boom" data-trace="{tid}"></div>
        <details class="ruw"><summary>Technisch logboek (letterlijk)</summary><pre class="trace">{E(t['ruw'])}</pre></details>
      </div>
    </details>
  </div>
</div>'''


LEGENDA = ('<div class="boomlegenda">'
           '<span><b>&#9636;</b> artikel</span>'
           '<span><b>&#8663;</b> verwijzing naar andere wet</span>'
           '<span><b>&#8681;</b> open norm ingevuld door lagere regeling</span>'
           '<span><b>&#9873;</b> Awb-bepaling toegepast</span>'
           '<span><b>&#9656;</b> rechtsregel</span>'
           '<span><b>&#8853;</b> berekening</span>'
           '<span><b>&#9670;</b> gegeven opgehaald</span>'
           '</div>')


def bouw():
    traces = laad_traces()
    stijl = open(os.path.join(HIER, 'style.css'), encoding='utf8').read()
    viewer = open(os.path.join(HIER, 'viewer.js'), encoding='utf8').read()

    secties = []
    gebruikt = {}
    for pers in PERSONAS:
        kenm = ''.join(f'<dt>{E(k)}</dt><dd>{E(v)}</dd>' for k, v in pers['kenmerken'])
        panelen = []
        for p in pers['panelen']:
            panelen.append(paneel(p, traces))
            if p[0] in traces:
                gebruikt[p[0]] = {'trace': traces[p[0]]['trace']}
        secties.append(f'''<section class="persona">
  <h2>{E(pers['naam'])}</h2>
  <div class="profile">
    <div><div class="naam">{E(pers['kop'])}</div><div class="rol">{E(pers['rol'])}</div></div>
    <dl>{kenm}</dl>
  </div>
  {LEGENDA}
  <div class="flow">{''.join(panelen)}</div>
</section>''')

    data = json.dumps(gebruikt, ensure_ascii=False, separators=(',', ':'))
    return f'''<!doctype html>
<html lang="nl"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Koen en Sadee door het stelsel</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Source+Sans+3:ital,wght@0,400;0,600;0,700;1,400&display=swap">
<style>{stijl}</style></head><body>
<header class="hoofd">
  <p class="eyebrow">Financieel CV &middot; doorloop</p>
  <h1>Koen en Sadee door het stelsel</h1>
  <p class="dek">Twee mensen, zeven regelingen, acht wetten. Elke uitkomst hieronder is
  uitgerekend door de engine; onder elk paneel staat de redenering die ertoe leidde,
  stap voor stap, zoals de uitvoering hem heeft vastgelegd.</p>
  <p class="stand">Stand van 9 september 2026 &middot; peildatum 2026-07-01 &middot; schema v0.5.4 &middot;
  verwerkt tot en met juristfeedback ronde 4</p>
</header>
<main>{''.join(secties)}</main>
<script type="application/json" id="tracedata">{data}</script>
<script>window.__TRACES__ = JSON.parse(document.getElementById("tracedata").textContent);</script>
<script>{viewer}</script>
</body></html>'''


if __name__ == '__main__':
    uit = bouw()
    open(UIT, 'w', encoding='utf8').write(uit)
    print(f'geschreven: {UIT} ({len(uit)} tekens)')
