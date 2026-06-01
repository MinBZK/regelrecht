import re, glob, sys
root = sys.argv[1] if len(sys.argv) > 1 else 'regulation'
law_outputs = {}
for path in glob.glob(f'{root}/**/*.yaml', recursive=True):
    txt = open(path).read()
    m = re.search(r'\$id:\s*(\S+)', txt)
    if not m: continue
    law_outputs[m.group(1)] = set(re.findall(r'- output: (\S+)', txt))
dangling, plain, ok = [], [], 0
for path in glob.glob(f'{root}/**/*.yaml', recursive=True):
    txt = open(path).read()
    m = re.search(r'\$id:\s*(\S+)', txt)
    if not m: continue
    fl = m.group(1)
    for sm in re.finditer(r'source:\s*\n\s+regulation:\s*(\S+)\s*\n\s+output:\s*(\S+)', txt):
        tl, to = sm.group(1), sm.group(2)
        if tl not in law_outputs or to not in law_outputs[tl]:
            dangling.append(f'{fl} -> {tl}.{to}')
        else:
            ok += 1
    for cm in re.finditer(r'conceptueel:', txt):
        if 'source:' not in txt[cm.start():cm.start()+300]:
            plain.append(fl)
print(f'clean={ok} dangling={len(dangling)} plain-param={len(plain)}')
for d in dangling: print('  DANGLING', d)
for p in plain: print('  PLAIN', p)
sys.exit(1 if (dangling or plain) else 0)
