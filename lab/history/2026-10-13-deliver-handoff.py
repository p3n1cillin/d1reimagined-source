"""Wait for the owned native test to stop, then use the existing report/outbox."""
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import time

private = Path(r'lab/history')
pointer = private/'2026-10-13-draft-pointer.json'
job = json.loads(pointer.read_text(encoding='utf-8'))
draft = Path(job['draft'])
runtime = Path(r'F:\d1\d1preservation-main')
receipt_path = private/'2026-10-13-daily-handoff-receipt.json'
if receipt_path.exists():
    raise SystemExit('An existing daily delivery receipt must be inspected instead of creating a duplicate')
capture_path = Path(r'F:\d1\d1xenia\tools\content_capture\capture.py')
config_path = Path(r'F:\d1\<pc>-import\d1xenia\local\content-capture\config.json')

def utc(): return datetime.now(timezone.utc).isoformat()
def native_running():
    # Read-only inventory; never stop or signal any process. A new run also defers transfer.
    result = subprocess.run(['tasklist','/FI','IMAGENAME eq shadPS4.exe','/FO','CSV','/NH'],
                            capture_output=True,text=True,check=True)
    return 'shadps4.exe' in result.stdout.lower()

job.update(state='WAITING_FOR_NATIVE_SAFE_CHECKPOINT',delivery_worker_started_utc=utc())
pointer.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')
deadline = time.monotonic()+3600
while native_running():
    if time.monotonic() >= deadline:
        job.update(state='DEFERRED_NATIVE_TEST_STILL_ACTIVE',checked_utc=utc())
        pointer.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')
        print(json.dumps({'state':job['state'],'draft_preserved':True}),flush=True)
        raise SystemExit(0)
    time.sleep(15)
# Allow the existing launcher to finish its receipt and child cleanup.
time.sleep(3)
run_receipt = runtime/'cfw-staging/contained-boot/20261013-173323-later-56578-worldset/receipt.json'
if not run_receipt.exists():
    job.update(state='DEFERRED_NATIVE_CLEANUP_RECEIPT_NOT_READY',checked_utc=utc())
    pointer.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'state':job['state'],'draft_preserved':True}),flush=True)
    raise SystemExit(0)
native = json.loads(run_receipt.read_text(encoding='utf-8-sig'))
if native.get('serversStopped') is not True or not native.get('endedUtc'):
    raise RuntimeError('Native launcher safe checkpoint not confirmed; draft preserved')

context = json.loads((draft/'metadata-input.json').read_text(encoding='utf-8'))
context.update(safe_checkpoint_utc=native['endedUtc'],
               handoff_delivery_prepared_utc=utc(),late_delivery=True,
               safe_checkpoint_basis='Owned native launcher has ended and records serversStopped true; no process was stopped by this handoff.')
# Do not turn post-cutoff observations into this edition's new evidence.
(draft/'metadata-input.json').write_text(json.dumps(context,indent=2)+'\n',encoding='utf-8')
with (draft/'daily-report.md').open('a',encoding='utf-8') as stream:
    stream.write('\nDelivery checkpoint: the existing native launcher ended at '+native['endedUtc']+
                 ' and its closed receipt records owned-server cleanup. This handoff did not stop it. '
                 'Its post-cutoff results remain excluded and roll to the next eligible edition.\n')

cfg = json.loads(config_path.read_text(encoding='utf-8-sig'))
cfg.update(CURRENT_GAME_BUILD='PS4 CUSA00219 owned build 56578.17.04.12.1251.evil; package metadata 01.33; synthetic lab interventions',
           CURRENT_SESSION='daily-engineering-handoff-2026-10-13',CURRENT_FEATURE='served-world-content-and-player-spawn')
report_config = draft/'config.report.json'
report_config.write_text(json.dumps(cfg,indent=2)+'\n',encoding='utf-8')

# The existing report command creates the durable verified package. No new packaging format.
command = [sys.executable,str(capture_path),'--config',str(report_config),'report',
           str(draft/'daily-report.md'),'--events',str(draft/'events.json'),
           '--metadata',str(draft/'metadata-input.json')]
result = subprocess.run(command,capture_output=True,text=True,check=True)
(draft/'report-command.stdout').write_text(result.stdout,encoding='utf-8')
(draft/'report-command.stderr').write_text(result.stderr,encoding='utf-8')
package = Path(json.loads(result.stdout.strip().splitlines()[-1]))
job.update(state='WAITING_FOR_NAS',package=str(package),clip_id=package.name)
pointer.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')

# Reuse the same verifier, transfer and retry machinery. Do not enqueue or resend media.
sys.path.insert(0,str(capture_path.parent))
spec = importlib.util.spec_from_file_location('daily_capture',capture_path)
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
cap = module.Capture(report_config)
sync_result = cap.sync(force=True)
state = cap.state(package.name)
if state.get('status') != 'SUBMITTED':
    job.update(state='DURABLE_OUTBOX_TRANSFER_PENDING',checked_utc=utc())
    pointer.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'state':job['state'],'clip_id':package.name,'sync':sync_result}),flush=True)
    raise SystemExit(2)
destination = Path(state['destination'])
local = cap.verify(package)
cap.verify(destination,local)
if (package/'hashes.json').read_bytes() != (destination/'hashes.json').read_bytes():
    raise RuntimeError('NAS manifest differs from local original')
checks = []
for name, pin in local['files'].items():
    payload = destination/name
    sha = hashlib.sha256(payload.read_bytes()).hexdigest()
    matches = sha == pin['sha256'] and payload.stat().st_size == pin['size']
    if not matches: raise RuntimeError('Independent NAS payload verification failed')
    checks.append({'file':name,'size':pin['size'],'sha256':sha,'matches':True})
pending = []
for path in cap.states.glob('*.json'):
    s = json.loads(path.read_text(encoding='utf-8'))
    if s.get('status') not in ['SUBMITTED','RETAINED_ON_NAS']:
        pending.append({'id':path.stem,'status':s.get('status')})
receipt = {'verified_at':utc(),'reporting_date':'2026-10-13',
           'daily_report':package.name,'destination':str(destination),
           'ready_verified':True,'manifest_bytes_match':True,'payloads':checks,
           'state':state,'new_clips':0,'new_screenshots':0,
           'media_decode':'NOT APPLICABLE: text-only','earlier_media_resent':False,
           'referenced_package_ids':context['referenced_package_ids'],
           'pending_outbox':pending,'native_safe_checkpoint_utc':native['endedUtc'],
           'source_checkpoint':context['tested_source_commit'],
           'publication_review_required':True,'handoff_complete':True}
receipt_path.write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
job.update(state='DELIVERED_AND_VERIFIED',destination=str(destination),verified_at=receipt['verified_at'])
pointer.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'state':job['state'],'clip_id':package.name,'destination':str(destination),'pending_outbox':pending}),flush=True)
