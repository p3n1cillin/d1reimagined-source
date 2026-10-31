"""Deliver the prepared text report at an input-idle checkpoint; keep native runs alive."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

PRIVATE = Path(__file__).parent
POINTER = PRIVATE/'2026-10-27-draft-pointer.json'
sys.path.insert(0, r'F:\d1\d1xenia\tools\content_capture')
from capture import Capture, lock, check_text


def utc():
    return datetime.now(timezone.utc).isoformat()


def critical_drivers():
    # Exclude both this Python and the inventory shell itself. A command-line
    # search which matches its own query is not a native-test observation.
    script = r'''$rows=@(Get-CimInstance Win32_Process | Where-Object {
      $_.ProcessId -ne $PID -and $_.ProcessId -ne EXCLUDE_PYTHON -and (
        ($_.Name -eq 'python.exe' -and $_.CommandLine -like '*d1*') -or
        ($_.Name -in @('pwsh.exe','powershell.exe') -and $_.CommandLine -match
          'd1-(walk|explore|fight|watchwrite)\.py|d1-agr-replay\.py|replay-divide\d+\.py')
      )} | Select-Object ProcessId,ParentProcessId,Name,CreationDate,CommandLine)
      ConvertTo-Json -InputObject $rows -Compress'''.replace('EXCLUDE_PYTHON', str(os.getpid()))
    result = subprocess.run(['pwsh','-NoProfile','-Command',script], capture_output=True,
                            text=True,check=True,timeout=15)
    return json.loads(result.stdout)


with lock(PRIVATE/'2026-10-27-delivery.lock',timeout=1):
    job=json.loads(POINTER.read_text())
    draft=Path(job['draft'])
    cfg=draft/'config.report.json'
    cap=Capture(cfg)
    def save():
        POINTER.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')
    parent=Path(cap.cfg['CONTENT_NAS_PATH'])/'incoming'/cap.machine/'2026/10/27'
    if not job.get('package'):
        for ready in parent.glob('*.ready'):
            meta=json.loads((ready/'metadata.json').read_text(encoding='utf-8-sig'))
            if meta.get('package_kind')=='engineering-report' and meta.get('reporting_date')=='2026-10-27':
                cap.verify(ready)
                job.update(state='EXISTING_DAILY_REPORT_VERIFIED',destination=str(ready),verified_at=utc())
                save();print(json.dumps(job));raise SystemExit(0)
    active=critical_drivers()
    if active:
        job.update(state='DEFERRED_CRITICAL_NATIVE_DRIVER',checked_utc=utc(),active_driver_inventory=active)
        save();print(json.dumps({'state':job['state'],'report_pid':os.getpid(),'report_parent':os.getppid(),'active':active}));raise SystemExit(0)
    if not job.get('package'):
        metadata=json.loads((draft/'metadata-input.json').read_text())
        metadata.update(native_safe_checkpoint_observed_at=utc(),
            safe_checkpoint_basis='Read-only process inventory: no other D1 Python/route/combat/watch driver; native game remains live and no process/input was changed by the report.')
        metadata['late_delivery']=datetime.now(timezone.utc)>datetime(2026,10,27,19,0,tzinfo=timezone.utc)
        metadata['within_requested_1930_deadline']=datetime.now(timezone.utc)<datetime(2026,10,27,19,30,tzinfo=timezone.utc)
        metadata['delivery_timing_basis']='Target19:00; active bounded native aim/capture allowed to finish. User deadline19:30. Post-cutoff gameplay excluded.'
        (draft/'metadata-input.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8')
        result=subprocess.run([sys.executable,r'F:\d1\d1xenia\tools\content_capture\capture.py',
            '--config',str(cfg),'report',str(draft/'daily-report.md'),'--events',str(draft/'events.json'),
            '--metadata',str(draft/'metadata-input.json')],capture_output=True,text=True,check=True,timeout=30)
        (draft/'report-command.stdout').write_text(result.stdout,encoding='utf-8')
        (draft/'report-command.stderr').write_text(result.stderr,encoding='utf-8')
        package=Path(json.loads(result.stdout.strip().splitlines()[-1]))
        job.update(state='WAITING_FOR_NAS',package=str(package),clip_id=package.name)
        save()
    package=Path(job['package'])
    cap.verify(package)
    cap.sync(force=True)
    state=cap.state(package.name)
    if state.get('status')!='SUBMITTED':
        job.update(state='DURABLE_OUTBOX_TRANSFER_PENDING',checked_utc=utc());save()
        print(json.dumps({'state':job['state'],'package':str(package)}));raise SystemExit(2)
    destination=Path(state['destination'])
    manifest=cap.verify(package)
    cap.verify(destination,manifest)
    assert (package/'hashes.json').read_bytes()==(destination/'hashes.json').read_bytes()
    payloads=[]
    for name,pin in manifest['files'].items():
        data=(destination/name).read_bytes()
        assert data and len(data)==pin['size'] and hashlib.sha256(data).hexdigest()==pin['sha256']
        payloads.append({'file':name,'size':pin['size'],'sha256':pin['sha256'],'matches':True})
    pending=[]
    for path in cap.states.glob('*.json'):
        value=json.loads(path.read_text())
        if value.get('status') in ('WAITING_FOR_NAS','FAILED_RETRYING','TRANSFERRING','VERIFYING'):
            pending.append({'id':path.stem,'status':value['status']})
    receipt={'verified_at':utc(),'reporting_date':'2026-10-27','daily_report':package.name,
        'destination':str(destination),'ready_verified':True,'manifest_bytes_match':True,
        'payloads':payloads,'transfer_started_at':state.get('transfer_started_at'),
        'submitted_at':state.get('submitted_at'),'new_screenshots':0,'new_clips':0,
        'video_decode':'NOT APPLICABLE: no new media','earlier_media_resent':False,
        'referenced_package_ids':json.loads((package/'metadata.json').read_text())['referenced_package_ids'],
        'nas_free_bytes_at_verification':shutil.disk_usage(cap.cfg['CONTENT_NAS_PATH']).free,
        'late_delivery':json.loads((package/'metadata.json').read_text()).get('late_delivery'),
        'within_requested_1930_deadline':json.loads((package/'metadata.json').read_text()).get('within_requested_1930_deadline'),
        'pending_outbox':pending,'publication_review_required':True,'handoff_complete':True}
    receipt_path=PRIVATE/'2026-10-27-daily-handoff-receipt.json'
    if receipt_path.exists():
        assert json.loads(receipt_path.read_text())['daily_report']==package.name
    else:
        with receipt_path.open('x',encoding='utf-8') as output:
            json.dump(receipt,output,indent=2)
    job.update(state='DELIVERED_AND_VERIFIED',destination=str(destination),verified_at=receipt['verified_at'])
    save()
    print(json.dumps({'state':job['state'],'clip_id':package.name,'payloads':len(payloads),
                      'pending_outbox':pending,'submitted_at':state.get('submitted_at')}))
