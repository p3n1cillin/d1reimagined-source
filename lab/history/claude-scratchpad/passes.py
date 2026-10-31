import sys, collections
ev = [l.split() for l in open(sys.argv[1])]
# group: a draw = run of T lines followed by RT/DB lines (BindResources precedes BeginRendering)
draws = []; cur_t = []
i = 0
while i < len(ev):
    e = ev[i]
    if e[0] == 'T':
        cur_t.append(e); i += 1; continue
    rts = []
    while i < len(ev) and ev[i][0] in ('RT', 'DB'):
        rts.append(ev[i]); i += 1
    sh = rts[0][1]
    draws.append((sh, cur_t, rts)); cur_t = []
# passes: consecutive draws with the same RT address set
passes = []
for sh, ts, rts in draws:
    key = tuple(sorted((r[0] + (r[2] if r[0] == 'RT' else '') , r[3] if r[0]=='RT' else r[2]) for r in rts))
    if passes and passes[-1][0] == key:
        p = passes[-1]
    else:
        p = [key, 0, collections.Counter(), collections.Counter(), set()]; passes.append(p)
    p[1] += 1; p[2][sh] += 1
    for t in ts: p[3][t[3]] += 1
    for r in rts:
        if r[0] == 'RT' and r[-1] == 'clearTrue': p[4].add('clear:' + r[3])
lim = int(sys.argv[2]) if len(sys.argv) > 2 else 400
for k, p in enumerate(passes[:lim]):
    tg = ' '.join(f'{a}={b}' for a, b in p[0])
    reads = ' '.join(sorted(a for a in p[3] if a.startswith(('36', '37', '38', '39'))))
    print(f'{k:3} n={p[1]:4} sh={len(p[2]):3} [{tg}] {" ".join(p[4])} reads: {reads}')
