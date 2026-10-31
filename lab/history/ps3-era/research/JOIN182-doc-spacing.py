from pathlib import Path
repo=Path('F:/d1/d1xenia')
replacements={
 'kind13':'kind 13','width0':'width 0','those96bits':'those 96 bits',
 'all175':'all 175','consume96bits':'consume 96 bits','omitting96bits':'omitting 96 bits',
 'and120bytes':'and 120 bytes','entry0':'entry 0','type1':'type 1','bucket14':'bucket 14',
 'objectives3/7/11/14':'objectives 3/7/11/14','targets87/65/78/88':'targets 87/65/78/88',
 'buckets14/3':'buckets 14/3','passes4timed/35reactive':'passes 4 timed/35 reactive',
 'on8October':'on 8 October','objective3state0':'objective 3 state 0',
 'type0':'type 0','No manual hostteleport24':'No manual host teleport to 24',
 '24:3,146':'24: 3,146','include1,364idle112':'include 1,364 idle112',
 'and1,425current112/target24':'and 1,425 current112/target24',
 'session6':'session 6','The70/45-second':'The 70/45-second',
 'identification8.49seconds':'identification 8.49 seconds',
 'indexed182segment':'indexed JOIN182 segment','failed182capture':'failed JOIN182 capture',
 'kind13 positions consume96bits':'kind 13 positions consume 96 bits',
 'every marker, shifting':'every marker, shifting',
 'with width0':'with width 0','zero xyz/w1':'zero xyz/w=1',
 'objective3 and':'objective 3 and','for112 and3 for24':'for 112 and 3 for 24'
}
for rel in ('notes/guardian-rises-reference-review.md','notes/guardian-rises-acceptance.md',
            'notes/feature-matrix.md','notes/checkpoint-2026-10-08-mission.md'):
    path=repo/rel
    data=path.read_text(encoding='utf-8')
    if 'checkpoint-' in rel:
        before,sep,data=data.partition('## JOIN181–182:')
        assert sep
    else:before=sep=''
    for old,new in replacements.items():data=data.replace(old,new)
    path.write_text(before+sep+data,encoding='utf-8')
