import hashlib,json,os,subprocess
from pathlib import Path
r=Path(r'lab/history\2026-10-28-sense-codex')
os.environ['D1_BUILD']='later-56578';os.environ['D1_BUILDS']=r'F:\d1\d1preservation-main\capture-rig\builds.json'
exe=r/'d1-oracle-sense.exe'
line=next(x.split('\tudt6\t')[1] for x in Path(r'lab/history\2026-10-28-sense\udt6-sense31.tsv').read_text(encoding='utf-8').splitlines() if '\tudt6\t' in x)
results=[]
for name,data,schema,ok in [('raw-valid',line,['12:0x80802c30'],True),('short','00',['12:0x80802c30'],False),('non-ascii','xx\u00e9',['12:0x80802c30'],False),('missing-kind',line,['1:0x80800357'],False),('conflict',line,['12:0x80802c30','12:0x80800357'],False)]:
 inp=r/(name+'.hex');out=r/(name+'.jsonl');inp.write_text(data,encoding='utf-8');args=[str(exe),'sensor','decode-sense','--input',str(inp),'--out',str(out)]
 for s in schema:args+=['--schema',s]
 p=subprocess.run(args,capture_output=True,text=True);assert (p.returncode==0)==ok,(name,p.stderr)
 results.append({'case':name,'exit':p.returncode,'pass':True})
pth=r/'raw-valid.jsonl';before=hashlib.sha256(pth.read_bytes()).hexdigest()
p=subprocess.run([str(exe),'sensor','decode-sense','--input',str(r/'raw-valid.hex'),'--out',str(pth),'--schema','12:0x80802c30'],capture_output=True,text=True)
assert p.returncode!=0 and hashlib.sha256(pth.read_bytes()).hexdigest()==before
results.append({'case':'existing-output-preserved','exit':p.returncode,'pass':True})
(r/'cli-controls.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results))
