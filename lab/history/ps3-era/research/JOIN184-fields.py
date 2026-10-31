import json, struct, re
from pathlib import Path
root=Path(__file__).parent
data=json.loads((root/'JOIN184-performance-analysis.json').read_text(encoding='utf-8'))
for tag,item in data['performances'].items():
    print(tag,'authored words')
    print(' '.join(f'{i*4:02x}:{word}' for i,word in enumerate(item['authored_words'])))
    for scene in item['scene_definitions']:
        raw=bytes.fromhex(scene['definition_bytes'])
        print('scene',scene['index'],hex(scene['handle']),hex(scene['offset']), ' '.join(f'{i*4:02x}:{word:08x}' for i,word in enumerate(struct.unpack('>48I',raw))))
for actor in data['actors']:
    print('actor',actor['index'])
    for variant in actor['most_common']:
        raw=bytes.fromhex(variant['bytes']);words=struct.unpack('>206I',raw)
        print('samples',variant['samples'],' '.join(f'{i*4:03x}:{v:08x}' for i,v in enumerate(words) if v))
native=(root.parent/'SIGNON_LOCAL/logs/JOIN184-pre-stop-native.log').read_text(encoding='utf-8',errors='replace')
for line in native.splitlines():
    if 'pc=0x42b51c' in line and 'm3=80cb8d' in line:
        raw=bytes.fromhex(re.search('m4=([0-9a-f]+)',line)[1])
        print('bind at', re.search(r'\d+:\d{2}:\d{2}\.\d{6}',line)[0], ' '.join(f'{i*4:02x}:{v:08x}' for i,v in enumerate(struct.unpack('>48I',raw))))
