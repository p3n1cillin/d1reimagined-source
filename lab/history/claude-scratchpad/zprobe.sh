cd /f/d1/tools; S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
timeout 40 python -I d1-watchwrite.py --seconds 4 --max-hits 1 --exec 0x77fb05 --peek "r15=" --peek rbp-0xb8 --peek rbp-0xbc --peek rbp-0xb4 --peek rbx+0x378 --peek rbx+0x380 --peek rbx+0x370 --out $S/zp.jsonl >/dev/null 2>&1
python -I -c "
import json,struct
for l in open(r'$S/zp.jsonl'):
    e=json.loads(l)
    if e.get('event')=='exec':
        p=e['peek']; f=lambda k: struct.unpack('<f',struct.pack('<I',int(p[k],16)))[0]
        print('level',p['r15='],'thr', [round(f(k),2) for k in ('rbp-0xb4','rbp-0xbc','rbp-0xb8')],'target',round(f('rbx+0x378'),2),'progress',round(f('rbx+0x380'),2),'0x370',round(f('rbx+0x370'),2))"
