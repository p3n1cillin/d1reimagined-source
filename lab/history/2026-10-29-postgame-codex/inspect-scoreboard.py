from pathlib import Path
import struct,json
P=Path(__file__).parent
b=(P/'owned-80aa3ee6.bin').read_bytes()
def arr(off,stride):
 n,rel=struct.unpack_from('<Qq',b,off)
 p=off+8+rel+16
 return [b[p+i*stride:p+(i+1)*stride] for i in range(n)]
def fnv(s):
 h=0x811c9dc5
 for c in s.encode():h=((h*0x1000193)&0xffffffff)^c
 return h
names=['kills','deaths','precision_kills','precisionKills','assists','score','totalActivityDurationSeconds','activityDurationSeconds','completed','time','completion','killsDeathsRatio','kills_deaths_ratio','precisionkills','precisionKillsPercentage','averageDeathDistance','averageKillDistance','opponentsDefeated','secondsPlayed','activityCompletions','activitiesEntered','killsDeathsAssists','killsDeathsAssistsRatio']
byhash={fnv(x):x for x in names}
for off,stride in [(0x20,32),(0x30,20),(0x40,20),(0x70,32),(0x80,32)]:
 print('ARRAY',hex(off))
 for i,rec in enumerate(arr(off,stride)):
  words=struct.unpack('<'+'I'*(stride//4),rec)
  print(i,rec.hex(' '),[byhash.get(x) for x in words])
print('hashes',{x:hex(fnv(x)) for x in names})
