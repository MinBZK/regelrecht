import re, yaml
from lxml import etree

def art_text(a):
    a = etree.fromstring(etree.tostring(a))
    for el in a.findall('.//meta-data'): el.getparent().remove(el)
    for el in a.findall('.//kop'): el.getparent().remove(el)
    for el in a.findall('.//redactie'):
        inner = re.sub(r'\s+',' ',' '.join(el.itertext())).strip()
        repl = etree.Element('x'); repl.text = f' [Red: {inner}] '
        el.getparent().replace(el, repl)
    return re.sub(r'\s+', ' ', ' '.join(a.itertext())).strip()

def fold(s, indent):
    words=s.split(); lines=[]; cur=''
    for w in words:
        if len(cur)+len(w)+1 > 100: lines.append(cur); cur=w
        else: cur = w if not cur else cur+' '+w
    if cur: lines.append(cur)
    pad=' '*indent
    return '\n'.join(pad+l for l in lines)

class _D(yaml.SafeDumper):
    def increase_indent(self, flow=False, indentless=False):
        return super().increase_indent(flow, False)

def mr_yaml(d, indent):
    s = yaml.dump(d, Dumper=_D, allow_unicode=True, sort_keys=False, default_flow_style=False, width=90)
    pad=' '*indent
    return '\n'.join(pad+l for l in s.rstrip('\n').split('\n'))

def emit(xml_path, out_path, slug, name, layer, bwb, pubdate, valid, url, mr={}):
    t = etree.parse(xml_path); arts=[]
    for a in t.findall('.//artikel'):
        nr = a.findtext('./kop/nr')
        if nr is None: continue
        arts.append((nr.strip(), art_text(a)))
    out=['---','$schema: >-','  https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.6/schema/v0.5.6/schema.json',
         f'$id: {slug}', f'name: {name}', f'regulatory_layer: {layer}', f'bwb_id: {bwb}',
         f"publication_date: '{pubdate}'", f"valid_from: '{valid}'", f'url: {url}', 'articles:']
    for nr, txt in arts:
        q = f"'{nr}'" if nr.isdigit() else nr
        out.append(f'  - number: {q}')
        out.append(f'    url: {url}#Artikel{nr}')
        out.append('    text: >-')
        out.append(fold(txt, 6))
        if nr in mr:
            out.append('    machine_readable:')
            out.append(mr_yaml(mr[nr], 6))
    open(out_path,'w').write('\n'.join(out)+'\n')
    print(out_path, len(arts), 'artikelen,', len(mr), 'machine_readable')

EC={'unit':'eurocent'}
def out_amt(n): return {'name':n,'type':'amount','type_spec':dict(EC)}
def src(n,t='amount'):
    d={'name':n,'type':t,'source':{'output':n}}
    if t=='amount': d['type_spec']=dict(EC)
    return d
def MR(execution, definitions=None):
    m={}
    if definitions: m['definitions']=definitions
    m['execution']=execution
    return m

WHT_MR = {
 '2': MR({'produces':{'legal_character':'WAARDEBEPALING','decision_type':'GEEN_BESLUIT'},
   'parameters':[{'name':'woont_met_partner_of_medebewoners','type':'boolean','required':True,
                  'description':'Feit: bewoont de huurder de woning samen met partner of een of meer medebewoners (onderhuurders tellen niet mee)'}],
   'output':[{'name':'is_meerpersoonshuishouden','type':'boolean'},{'name':'is_eenpersoonshuishouden','type':'boolean'}],
   'actions':[
     {'output':'is_meerpersoonshuishouden','value':'$woont_met_partner_of_medebewoners'},
     {'output':'is_eenpersoonshuishouden','operation':'NOT','value':'$woont_met_partner_of_medebewoners'}]}),
 '5': MR({'produces':{'legal_character':'WAARDEBEPALING','decision_type':'GEEN_BESLUIT'},
   'parameters':[
     {'name':'huurprijs','type':'amount','required':True,'type_spec':dict(EC),
      'description':'Art. 1(d): prijs voor het enkele gebruik van de woning, per maand'},
     {'name':'maximale_huurprijsgrens_uhw','type':'amount','required':True,'type_spec':dict(EC),
      'description':'Maximale huurprijsgrens krachtens art. 10 lid 1 en 12 lid 2 Uitvoeringswet huurprijzen woonruimte; lid 2 stelt een procedurele voorwaarde (advies huurcommissie) die hier niet is gemodelleerd'}],
   'output':[out_amt('rekenhuur')],
   'actions':[{'output':'rekenhuur','operation':'MIN','values':['$huurprijs','$maximale_huurprijsgrens_uhw']}]}),
 '13': MR({'produces':{'legal_character':'WAARDEBEPALING','decision_type':'GEEN_BESLUIT'},
   'parameters':[
     {'name':'bewoner_21_of_ouder_of_kind_in_woning','type':'boolean','required':True,
      'description':'Art. 13 lid 1 onder a sub 1: huurder, partner of medebewoner is 21 of ouder, dan wel deelt de woning met een kind of pleegkind'},
     {'name':'jonger_dan_21_met_handicap','type':'boolean','required':True,
      'description':'Art. 13 lid 1 onder a sub 2'}],
   'output':[out_amt('maximale_huurgrens')],
   'actions':[{'output':'maximale_huurgrens','operation':'IF','cases':[
       {'when':{'operation':'OR','conditions':[
           {'operation':'EQUALS','subject':'$bewoner_21_of_ouder_of_kind_in_woning','value':True},
           {'operation':'EQUALS','subject':'$jonger_dan_21_met_handicap','value':True}]},
        'then':'$MAXIMALE_HUURGRENS_HOOG'}],
     'default':'$MAXIMALE_HUURGRENS_LAAG'}]},
   {'MAXIMALE_HUURGRENS_HOOG':{'value':93293,'description':'Art. 13 lid 1 onder a: EUR 932,93 per maand (geindexeerd Stcrt. 2025, 39783)'},
    'MAXIMALE_HUURGRENS_LAAG':{'value':49820,'description':'Art. 13 lid 1 onder b: EUR 498,20 per maand'}}),
 '16': MR({'produces':{'legal_character':'WAARDEBEPALING','decision_type':'GEEN_BESLUIT'},
   'input':[src('normhuur'),
            {'name':'verlaging_basishuur','type':'amount','source':{'regulation':'wet_verlaging_eigen_bijdrage_huurtoeslag','output':'verlaging_basishuur'},'type_spec':dict(EC)}],
   'output':[out_amt('basishuur')],
   'actions':[{'output':'basishuur','operation':'SUBTRACT','values':[
       {'operation':'ADD','values':['$normhuur','$OPSLAG_BASISHUUR']},'$verlaging_basishuur']}]},
   {'OPSLAG_BASISHUUR':{'value':0,'description':'Art. 16: "verhoogd met EUR 12"; redactionele correctie bij Stb. 2022/528: per 1 januari 2023 EUR 0'}}),
 '17': MR({'produces':{'legal_character':'WAARDEBEPALING','decision_type':'GEEN_BESLUIT'},
   'parameters':[
     {'name':'aow_bruto_jaarbedrag_alleenstaande','type':'amount','required':True,'type_spec':dict(EC),
      'description':'Bruto-ouderdomspensioen art. 9 lid 1 onder a AOW plus vakantie-uitkering art. 29 lid 1 onder a AOW, naar redelijke verwachting in het berekeningsjaar, herrekend naar een jaarinkomen; de herrekening laat de wet open en wordt als gegeven aangeleverd'},
     {'name':'aow_bruto_jaarbedrag_gehuwde','type':'amount','required':True,'type_spec':dict(EC),
      'description':'Idem, art. 9 lid 1 onder b en art. 29 lid 1 onder b AOW (bedrag per gehuwde pensioengerechtigde)'}],
   'input':[src('is_meerpersoonshuishouden','boolean')],
   'output':[out_amt('minimum_inkomensijkpunt'),out_amt('normhuur')],
   'actions':[
     {'output':'minimum_inkomensijkpunt','operation':'IF','cases':[
        {'when':{'operation':'EQUALS','subject':'$is_meerpersoonshuishouden','value':True},
         'then':{'operation':'ADD','values':[{'operation':'MULTIPLY','values':[2,'$aow_bruto_jaarbedrag_gehuwde']},'$OPHOGING_MEERPERSOONS']}}],
      'default':{'operation':'ADD','values':['$aow_bruto_jaarbedrag_alleenstaande','$OPHOGING_EENPERSOONS']}},
     {'output':'normhuur','operation':'IF','cases':[
        {'when':{'operation':'EQUALS','subject':'$is_meerpersoonshuishouden','value':True},
         'then':{'operation':'SUBTRACT','values':['$NORMHUUR_BIJ_IJKPUNT','$VERLAGING_MEERPERSOONS']}}],
      'default':{'operation':'SUBTRACT','values':['$NORMHUUR_BIJ_IJKPUNT','$VERLAGING_EENPERSOONS']}}]},
   {'OPHOGING_EENPERSOONS':{'value':234000,'description':'Art. 17 lid 1 onder a: EUR 2 340'},
    'OPHOGING_MEERPERSOONS':{'value':251200,'description':'Art. 17 lid 1 onder b: EUR 2 512'},
    'NORMHUUR_BIJ_IJKPUNT':{'value':25249,'description':'Art. 17 lid 2: EUR 252,49 (geindexeerd Stcrt. 2025, 39783)'},
    'VERLAGING_EENPERSOONS':{'value':182,'description':'Art. 17 lid 3 onder a: EUR 1,82'},
    'VERLAGING_MEERPERSOONS':{'value':363,'description':'Art. 17 lid 3 onder b: EUR 3,63'}}),
 '20': MR({'produces':{'legal_character':'WAARDEBEPALING','decision_type':'GEEN_BESLUIT'},
   'parameters':[{'name':'huishouden_drie_of_meer_personen','type':'boolean','required':True,
                  'description':'Art. 20 lid 2: bestaat het huishouden (afgezien van onderhuurders c.s.) uit drie of meer personen'}],
   'output':[out_amt('kwaliteitskortingsgrens'),out_amt('aftoppingsgrens')],
   'actions':[
     {'output':'kwaliteitskortingsgrens','value':'$KWALITEITSKORTINGSGRENS'},
     {'output':'aftoppingsgrens','operation':'IF','cases':[
        {'when':{'operation':'EQUALS','subject':'$huishouden_drie_of_meer_personen','value':True},
         'then':'$AFTOPPINGSGRENS_HOOG'}],
      'default':'$AFTOPPINGSGRENS_LAAG'}]},
   {'KWALITEITSKORTINGSGRENS':{'value':49820,'description':'Art. 20 lid 1: EUR 498,20 per maand'},
    'AFTOPPINGSGRENS_LAAG':{'value':71302,'description':'Art. 20 lid 2 onder a: EUR 713,02 (een of twee personen)'},
    'AFTOPPINGSGRENS_HOOG':{'value':76414,'description':'Art. 20 lid 2 onder b: EUR 764,14 (drie of meer personen)'}}),
 '21': MR({'produces':{'legal_character':'BESCHIKKING','decision_type':'TOEKENNING'},
   'parameters':[{'name':'rekeninkomen','type':'amount','required':True,'type_spec':dict(EC),
                  'description':'Art. 1(i): gezamenlijke toetsingsinkomens (art. 8 Awir), per jaar'}],
   'input':[src('rekenhuur'),src('basishuur'),src('kwaliteitskortingsgrens'),src('aftoppingsgrens'),
            src('maximale_huurgrens'),src('minimum_inkomensijkpunt'),
            src('is_meerpersoonshuishouden','boolean'),
            {'name':'percentage_kwaliteitskorting','type':'number','source':{'regulation':'besluit_op_de_huurtoeslag','output':'percentage_kwaliteitskorting'}},
            {'name':'percentage_boven_aftoppingsgrens','type':'number','source':{'regulation':'besluit_op_de_huurtoeslag','output':'percentage_boven_aftoppingsgrens'}}],
   'output':[out_amt('hoogte_huurtoeslag')],
   'actions':[{'output':'hoogte_huurtoeslag',
     'operation':'ROUND','precision':0,'value':
     {'operation':'MAX','values':[0,
       {'operation':'SUBTRACT','values':[
         {'operation':'ADD','values':[
           {'operation':'MAX','values':[0,{'operation':'SUBTRACT','values':[
               {'operation':'MIN','values':['$rekenhuur','$kwaliteitskortingsgrens']},'$basishuur']}]},
           {'operation':'ADD','values':[
             {'operation':'MULTIPLY','values':['$percentage_kwaliteitskorting',
               {'operation':'MAX','values':[0,{'operation':'SUBTRACT','values':[
                   {'operation':'MIN','values':['$rekenhuur','$aftoppingsgrens']},'$kwaliteitskortingsgrens']}]}]},
             {'operation':'MULTIPLY','values':['$percentage_boven_aftoppingsgrens',
               {'operation':'MAX','values':[0,{'operation':'SUBTRACT','values':[
                   {'operation':'MIN','values':['$rekenhuur','$maximale_huurgrens']},'$aftoppingsgrens']}]}]}]}]},
         {'operation':'MULTIPLY','values':[
           {'operation':'MAX','values':[0,{'operation':'SUBTRACT','values':['$rekeninkomen','$minimum_inkomensijkpunt']}]},
           {'operation':'DIVIDE','values':[
             {'operation':'IF','cases':[
               {'when':{'operation':'EQUALS','subject':'$is_meerpersoonshuishouden','value':True},
                'then':'$AFBOUWPERCENTAGE_MEERPERSOONS'}],
              'default':'$AFBOUWPERCENTAGE_EENPERSOONS'},
             '$MAANDEN_PER_JAAR']}]}]}]}}]},
   {'AFBOUWPERCENTAGE_EENPERSOONS':{'value':0.27,'description':'Art. 21 lid 2 onder a: 27%'},
    'AFBOUWPERCENTAGE_MEERPERSOONS':{'value':0.22,'description':'Art. 21 lid 2 onder b: 22%'},
    'MAANDEN_PER_JAAR':{'value':12,'description':'Art. 21 lid 2: afbouwpercentage/12'}}),
}

WVEB_MR = {'1': MR({'produces':{'legal_character':'WAARDEBEPALING','decision_type':'GEEN_BESLUIT'},
   'output':[{'name':'verlaging_basishuur','type':'amount','type_spec':{'unit':'eurocent'}}],
   'actions':[{'output':'verlaging_basishuur','value':'$VERLAGING_2026'}]},
   {'VERLAGING_2026':{'value':4815,'description':'Art. 1 onder c: in 2026 verlaagd met EUR 48,15; deze corpusversie (valid_from 2026-01-01) codeert het bedrag voor 2026'}})}

BHT_MR = {'7': MR({'produces':{'legal_character':'WAARDEBEPALING','decision_type':'GEEN_BESLUIT'},
   'output':[{'name':'percentage_kwaliteitskorting','type':'number'},
             {'name':'percentage_boven_aftoppingsgrens','type':'number'}],
   'actions':[
     {'output':'percentage_kwaliteitskorting','value':'$PERCENTAGE_B'},
     {'output':'percentage_boven_aftoppingsgrens','value':'$PERCENTAGE_C'}]},
   {'PERCENTAGE_B':{'value':0.65,'description':'Art. 7 lid 1: het percentage bedoeld in art. 21 lid 1 onder b van de wet is 65'},
    'PERCENTAGE_C':{'value':0.4,'description':'Art. 7 lid 2: het percentage bedoeld in art. 21 lid 1 onder c van de wet is 40'}})}

def fix_if_actions(mrmap):
    for m in mrmap.values():
        acts=m.get('execution',{}).get('actions',[])
        for i,a in enumerate(acts):
            if a.get('operation')=='IF':
                acts[i]={'output':a['output'],'value':{k:v for k,v in a.items() if k!='output'}}
for _m in (WHT_MR, WVEB_MR, BHT_MR): fix_if_actions(_m)

emit('/tmp/wht2026.xml','corpus/regulation/nl/wet/wet_op_de_huurtoeslag/2026-01-01.yaml',
     'wet_op_de_huurtoeslag','Wet op de huurtoeslag','WET','BWBR0008659','2024-12-20','2026-01-01',
     'https://wetten.overheid.nl/BWBR0008659/2026-01-01', WHT_MR)
emit('/tmp/wveb.xml','corpus/regulation/nl/wet/wet_verlaging_eigen_bijdrage_huurtoeslag/2026-01-01.yaml',
     'wet_verlaging_eigen_bijdrage_huurtoeslag','Wet verlaging eigen bijdrage huurtoeslag','WET','BWBR0049106','2023-12-28','2026-01-01',
     'https://wetten.overheid.nl/BWBR0049106/2026-01-01', WVEB_MR)
emit('/tmp/bht.xml','corpus/regulation/nl/amvb/besluit_op_de_huurtoeslag/2026-01-01.yaml',
     'besluit_op_de_huurtoeslag','Besluit op de huurtoeslag','AMVB','BWBR0008763','1997-06-26','2026-01-01',
     'https://wetten.overheid.nl/BWBR0008763/2026-01-01', BHT_MR)
