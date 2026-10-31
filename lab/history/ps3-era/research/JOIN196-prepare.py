from pathlib import Path
import json
rt=Path(__file__).parent.parent
research=rt/'RESEARCH'
for suffix in ('candidate.py','prepare.ps1'):
    path=research/f'JOIN196-{suffix}'
    assert not path.exists(),path
candidate=(research/'JOIN195-candidate.py').read_text(encoding='utf-8-sig').replace('JOIN195','JOIN196')
candidate=candidate.replace("{'0xa':42,'0xc':2706}","{'0xa':42}")
candidate=candidate.replace(" and item[12]==2706", " and 12 not in item")
(research/'JOIN196-candidate.py').write_text(candidate,encoding='utf-8')
prepare=(research/'JOIN195-prepare.ps1').read_text(encoding='utf-8-sig').replace('JOIN195','JOIN196')
prepare=prepare.replace('logs\\JOIN194-cleanup-result.json','logs\\JOIN195-cleanup-result.json')
prepare=prepare.replace('SIGNON_LOCAL\\JOIN194-launch.ps1','SIGNON_LOCAL\\JOIN195-launch.ps1')
prepare=prepare.replace(".Replace('JOIN194','JOIN196')",".Replace('JOIN195','JOIN196')")
prepare=prepare.replace("$taskText=$taskText.Replace(","$taskText=$taskText.Replace(")
start=prepare.index("$taskTrace=@'")
end=prepare.index("'@",start)+2
trace="""$taskTrace=@'
# JOIN196: appearance-only; distinguish Ghost unit, cosmetic override, and returned cache ID.
71ec90 16 r3:48
71edcc 16 r1+0x74:4 r26:12
42fa70 8 r20:824
2d184c 8 r5:260
2d1880 8 r1+0x70:4
42fae0 8 r20:824
c67e8 12 if=lr==0x42fa78 r3:12
42b51c 4 r3:208
'@"""
prepare=prepare[:start]+trace+prepare[end:]
prepare=prepare.replace("model arrangement=2706; no renderer change.","no model arrangement override; distinguish unit/cosmetic/cache; no renderer change.")
(research/'JOIN196-prepare.ps1').write_text(prepare,encoding='utf-8')
print(research/'JOIN196-prepare.ps1')
