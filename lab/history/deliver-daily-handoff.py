"""Private one-shot delivery adapter over the existing capture report/outbox tools."""
import argparse
import csv
from datetime import datetime, timezone
import errno
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

parser=argparse.ArgumentParser()
parser.add_argument('--date',required=True)
args=parser.parse_args()
datetime.strptime(args.date,'%Y-%m-%d')
private=Path(r'lab/history')
pointer=private/(args.date+'-draft-pointer.json')
receipt_path=private/(args.date+'-daily-handoff-receipt.json')
job=json.loads(pointer.read_text(encoding='utf-8'))
draft=Path(job['draft'])
source=Path(r'F:\d1\d1xenia\tools\content_capture')
sys.path.insert(0,str(source))
from capture import Capture, lock, check_text

def utc():return datetime.now(timezone.utc).isoformat()
def save():pointer.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')
def running_native():
    result=subprocess.run(['tasklist','/FO','CSV','/NH'],capture_output=True,text=True,errors='replace',timeout=10,check=True)
    return [r[0] for r in csv.reader(result.stdout.splitlines())
            if r and r[0].lower() in ('shadps4.exe','rpcs3.exe')]

with lock(private/(args.date+'-delivery.lock'),timeout=1):
    cfg_path=draft/'config.report.json'
    cfg=json.loads(cfg_path.read_text(encoding='utf-8-sig'))
    for name in ['CURRENT_GAME_BUILD','CURRENT_SESSION','CURRENT_FEATURE']:
        os.environ[name]=cfg[name]
    cap=Capture(cfg_path)
    # Daily deduplication precedes package creation. Never replace an existing ready report.
    date_parts=args.date.split('-')
    nas_parent=Path(cfg['CONTENT_NAS_PATH'])/'incoming'/cap.machine/Path(*date_parts)
    if not job.get('package') and nas_parent.exists():
        for ready in nas_parent.glob('*.ready'):
            meta=json.loads((ready/'metadata.json').read_text(encoding='utf-8-sig'))
            if meta.get('package_kind')=='engineering-report' and meta.get('reporting_date')==args.date:
                cap.verify(ready)
                job.update(state='EXISTING_DAILY_REPORT_VERIFIED',destination=str(ready),verified_at=utc())
                save();print(json.dumps(job));raise SystemExit(0)
    if not job.get('package'):
        job.update(state='WAITING_FOR_NATIVE_SAFE_CHECKPOINT',delivery_worker_started_utc=utc());save()
        deadline=time.monotonic()+3600
        while True:
            if time.monotonic()>deadline:
                job.update(state='DEFERRED_NATIVE_TEST_STILL_ACTIVE',checked_utc=utc());save()
                print(json.dumps(job));raise SystemExit(0)
            if running_native():
                time.sleep(5)
                continue
            time.sleep(2)
            if not running_native():
                break
            job.update(state='WAITING_FOR_NATIVE_SAFE_CHECKPOINT',native_restart_observed_at=utc());save()
        metadata=json.loads((draft/'metadata-input.json').read_text(encoding='utf-8'))
        metadata.update(native_safe_checkpoint_observed_at=utc(),
                        safe_checkpoint_basis='Read-only native process inventory is empty; no emulator or server was stopped by this handoff.')
        for name, expected in metadata.get('handoff_tool_source_sha256',{}).items():
            p=Path(r'F:\d1\d1xenia')/name
            if hashlib.sha256(p.read_bytes()).hexdigest()!=expected:
                raise RuntimeError('Handoff tool source changed after validation; draft retained')
        for item in json.loads((draft/'selected-screenshots.json').read_text(encoding='utf-8')):
            if hashlib.sha256(Path(item['source_path']).read_bytes()).hexdigest()!=item['source_sha256']:
                raise RuntimeError('Reviewed original changed; draft retained')
        (draft/'metadata-input.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8')
        for name in ['daily-report.md','events.json','metadata-input.json']:
            check_text((draft/name).read_text(encoding='utf-8'))
        command=[sys.executable,str(source/'capture.py'),'--config',str(cfg_path),'report',
                 str(draft/'daily-report.md'),'--events',str(draft/'events.json'),
                 '--metadata',str(draft/'metadata-input.json')]
        for item in json.loads((draft/'selected-screenshots.json').read_text(encoding='utf-8')):
            command.extend(['--reviewed-screenshot',item['source_path']])
        result=subprocess.run(command,capture_output=True,text=True,check=True)
        (draft/'report-command.stdout').write_text(result.stdout,encoding='utf-8')
        (draft/'report-command.stderr').write_text(result.stderr,encoding='utf-8')
        package=Path(json.loads(result.stdout.strip().splitlines()[-1]))
        job.update(state='WAITING_FOR_NAS',package=str(package),clip_id=package.name);save()
    package=Path(job['package'])
    deadline=time.monotonic()+120
    while cap.state(package.name).get('status')!='SUBMITTED':
        try:
            cap.sync(force=True)
        except OSError as error:
            if error.errno not in (errno.EACCES,errno.EAGAIN,errno.EDEADLK):raise
        if cap.state(package.name).get('status')=='SUBMITTED':break
        if time.monotonic()>deadline:
            job.update(state='DURABLE_OUTBOX_TRANSFER_PENDING',checked_utc=utc());save()
            print(json.dumps({'state':job['state'],'package':str(package)}));raise SystemExit(2)
        time.sleep(2)
    state=cap.state(package.name)
    destination=Path(state['destination'])
    manifest=cap.verify(package)
    cap.verify(destination,manifest)
    if (package/'hashes.json').read_bytes()!=(destination/'hashes.json').read_bytes():
        raise RuntimeError('NAS manifest is not byte-identical')
    payloads=[]
    for name,pin in manifest['files'].items():
        p=destination/name
        actual=hashlib.sha256(p.read_bytes()).hexdigest()
        if p.stat().st_size!=pin['size'] or actual!=pin['sha256']:
            raise RuntimeError('Independent NAS payload verification failed')
        payloads.append({'file':name,'size':pin['size'],'sha256':actual,'matches':True})
    selected=json.loads((draft/'selected-screenshots.json').read_text(encoding='utf-8'))
    for shot in selected:
        if hashlib.sha256((destination/shot['file']).read_bytes()).hexdigest()!=shot['source_sha256']:
            raise RuntimeError('Screenshot differs from inspected original')
    metadata=json.loads((package/'metadata.json').read_text(encoding='utf-8'))
    pending=[];terminal=[]
    for path in cap.states.glob('*.json'):
        s=json.loads(path.read_text())
        if s.get('status') in ['WAITING_FOR_NAS','FAILED_RETRYING','TRANSFERRING','VERIFYING']:
            pending.append({'id':path.stem,'status':s['status']})
        elif s.get('status')=='SUPERSEDED':terminal.append(path.stem)
    receipt={'verified_at':utc(),'reporting_date':args.date,'daily_report':package.name,
             'destination':str(destination),'ready_verified':True,'manifest_bytes_match':True,
             'payloads':payloads,'transfer_started_at':state.get('transfer_started_at'),
             'submitted_at':state.get('submitted_at'),'new_screenshots':len(selected),'new_clips':0,
             'screenshot_bytes_match_inspected_originals':True if selected else None,
             'screenshot_decode':'PASS' if selected else 'NOT APPLICABLE: no screenshots',
             'video_decode':'NOT APPLICABLE: no video',
             'earlier_media_resent':False,'referenced_package_ids':metadata['referenced_package_ids'],
             'nas_free_bytes_at_verification':shutil.disk_usage(cfg['CONTENT_NAS_PATH']).free,
             'pending_outbox':pending,'terminal_superseded_not_queued':terminal,
             'late_delivery':metadata.get('late_delivery',False),'source_checkpoint':metadata['tested_source_commit'],
             'publication_review_required':True,'handoff_complete':True}
    if receipt_path.exists():
        old=json.loads(receipt_path.read_text())
        if old.get('daily_report')!=package.name:raise RuntimeError('Different daily receipt already exists')
    else:receipt_path.write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    job.update(state='DELIVERED_AND_VERIFIED',destination=str(destination),verified_at=receipt['verified_at']);save()
    print(json.dumps({'state':job['state'],'clip_id':package.name,'payloads':len(payloads),
                      'screenshots':len(selected),'pending_outbox':pending,'submitted_at':state.get('submitted_at')}))
