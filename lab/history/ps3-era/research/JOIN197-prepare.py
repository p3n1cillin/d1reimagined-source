from pathlib import Path
rt=Path(__file__).parent.parent
research=rt/'RESEARCH'
candidate=(research/'JOIN196-candidate.py').read_text(encoding='utf-8-sig').replace('JOIN196','JOIN197')
candidate=candidate.replace("{'0xa':42}","{'0xa':42,'0x18':{'0':{'0':14,'2':1}}}")
candidate=candidate.replace(' and 12 not in item,item',' and 12 not in item and item[24][0]=={0:14,2:1},item')
candidate=candidate.replace('for k in (0,8,10,12)','for k in (0,8,10,12,24)')
candidate=candidate.replace('Private appearance-only candidate; no world or Ghost visibility acceptance.',
    'Private appearance plus Ghost channel 14/default cosmetic index 1 candidate; semantics and visibility require native confirmation.')
path=research/'JOIN197-candidate.py'
with path.open('x',encoding='utf-8') as stream:stream.write(candidate)
prepare=(research/'JOIN196-prepare.ps1').read_text(encoding='utf-8-sig').replace('JOIN196','JOIN197').replace('JOIN195','JOIN196')
prepare=prepare.replace('2d184c 8 r5:260','71ee68 16 r1+0x7c:4 r23:4\n2d184c 8 r5:260')
prepare=prepare.replace('appearance-only; distinguish Ghost unit, cosmetic override, and returned cache ID.',
    'appearance=42 plus private Ghost channel 14/cosmetic index 1; distinguish unit, override, cache.')
prepare=prepare.replace('no model arrangement override; distinguish unit/cosmetic/cache; no renderer change.',
    'Ghost channel 14/cosmetic index 1 candidate; no model arrangement override or renderer change.')
path=research/'JOIN197-prepare.ps1'
with path.open('x',encoding='utf-8') as stream:stream.write(prepare)
print(path)
