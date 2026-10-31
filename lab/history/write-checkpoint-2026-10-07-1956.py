from pathlib import Path
from datetime import datetime, timezone
import json
import subprocess

root=Path(r'lab/history')
now=datetime.now(timezone.utc).isoformat()
git=lambda cwd,*args:subprocess.check_output(['git','-C',cwd,*args],text=True).strip()
source=git(r'F:\d1\d1xenia','rev-parse','HEAD')
data=git(r'F:\d1\d1xenia-data','rev-parse','HEAD')
assert source=='d3ae32bf2c659afd1059625a77842d902e2a4971'
assert data=='a3b316bafa6ff72f75549365d8b7b4f37cc97c1e'
assert not git(r'F:\d1\d1xenia','status','--porcelain')
assert not git(r'F:\d1\d1xenia-data','status','--porcelain')
checkpoint={
 'at':now,'previous_goal_turn':'PROGRESS','current_goal_turn':'PROGRESS','goal_status':'active/incomplete',
 'objective_read':'Full goal-objective.md read at current goal-turn start; full-game requirements remain intact.',
 'source_commit':source,'data_commit':data,'remote_heads_verified':True,'public_worktrees_clean':True,
 'runtime_build':'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf',
 'pdb':'5a2307915a59888edcc3722b8831c5b89b49aa272d293e26a192d857141909cf',
 'private_renderer_dirty_entries':53,'unrelated_work_preserved':True,
 'verification':['52 actual-source CPU checks PASS, independently rerun; actual log final-write timestamp recorded separately.','One incremental Release build PASS before JOIN173, deployed bytes match. No renderer build while native runs live.','LF Git patch application with --check --whitespace=error-all PASS; all3 modified files reproduce with normalized newlines. Initial CRLF fixture failure retained and corrected.'],
 'cpu_rerun_log_final_write_utc':datetime.fromtimestamp((root.parent/'RESEARCH/JOIN173-ppu-filter-audit/verification-1928.log').stat().st_mtime,timezone.utc).isoformat(),
 'native173':'CLOSED; cleanup19:08:36.2388945Z and all exact identities reverified.1052 samples,432 loaded/inactive112 across2 attempts,244 in original attempt, no idle112, no lookup/recheck errors.17 subtype21 direct attempts,8 unique gate/result combinations. Gates1/8, actual shared epilogues return1 for34-byte state payload; original connection receives no21 after112 request. Outer direct return remains unconditional. Clock advances; live reset19:04:04.398, separate shutdown19:08:35.870. No state37/38 sampled.18 indexed originals hash-match,7 unindexed failures retained,last2 fullDecodePASS39.24/20.82s1280x720/audio. Inspected samples loading/contacting-server. Wrong initial loaded-word-as-list count0 superseded by bitset count432.',
 'native174':'CLOSED; wrapper75393 exit0; cleanup19:40:51.2242143Z. Exact client23120/ticks639269981157421235 plus services/proxy3032/probe2672 all reverified CLOSED.1017 samples,326 loaded/inactive112 across2 attempts,273 original attempt, no idle112 or lookup/recheck errors.16 subtype21 allocator/post-allocation entries;2 matching initialized47-byte record prefixes at allocation and consumer. First native6:10.710615/711634, second11:05.481820/482848. This proves allocation/consumer entry, not final packet/header/socket/receipt. Original request19:34:45.825, reset19:36:49.242, no subsequent21 on original.20 indexed original hashes match,3 unindexed retained,last2fullDecodePASS30.33/16.5s1280x720/audio; inspected final samples loading.',
 'native_limits':'Core layer verified before each stop, bounded logs0VUID/0device-loss/0fatal. No RSX capture armed. This is no graphics, control, combat or mission-completion acceptance. Original title/watch and XInput restored, controls[], RPCN preserved, both cleanup errors[].',
 'input_correction':'In174, dot-sourcing Send-RpcsKey.ps1 inadvertently invokes defaultX150ms; subsequent function call fails because it is a script. Sole owned RPCS3 is live at surrounding checks; no400ms/identity-validated helper press claimed. Images urgent/outdoor before, black/loading after. No walking. Future invoke & script with explicit PID+identity; do not dot-source it.',
 'selection_correction':'Actual kind17 mapper returns codec index8; previous codec17 private inspection is invalid for this path. Correct codec object1ad5e9c/vtable13e9f78, serializer47837c, descriptor selector7fce0 returns3. Factory3 object1adb29c/vtable13ed200, headerencoder5b2610->8c8220. Non-atomic174 runtime method receipts are preserved; no next-stage native result yet.',
 'next_task':'Fresh JOIN175 from same C5 build/fixture: initialized full47-byte subtype21 header+34 payload at post-allocation hook, decode actual current/target with schema, retain allocation/consumer match; trace factory3 headerencoding and actual return helper, then forwarding queue/socket receipt. Planned watch file exists but is NOT armed. Verify shared-helper entry emissions before interpreting return; inspect8c8220 failure gates. No synthetic ACK, VM state force or direct112 bypass.',
 'unmet_full_goal':'Stable combined character/equipment/outdoor/mission fidelity; reliable native113-to112 activation, visible consumed player inputs, firstmission combat/objectives/rewards/checkpoints/completion/Tower; full destination/activity/AI/combat/progression/vendor/quest/unlock/UI/persistence/backend/fireteam/multiplayer coverage.',
 'retired_xbox':'Exact historical xenia-netplay-src is clean Xbox source, still present. Authorized native PowerShell deletion rejected before execution: blocked by policy. No bypass/retry using another shell. Both READMEs corrected; private rejection record retained. This does not block RPCS3 engineering.',
 'daily_handoff':'Oct7 already delivered/verified17:49UTC; immutable package remains unchanged. All later evidence rolls to next eligible edition with actual dates. No new consolidated daily report, final-video edit or publication this turn.'
}
path=root/'2026-10-07-goal-checkpoint-1956.json'
with path.open('x',encoding='utf-8') as stream:json.dump(checkpoint,stream,indent=2)
progress=root/'2026-10-07-progress.md'
intro=f'## Current safe engineering checkpoint — {now}\n\n'
intro+='Previous and current goal engineering turns: PROGRESS. Whole-game goal remains ACTIVE and incomplete. Two completed native tests narrow the failure from readiness/outer returns to BAP forwarding, after actual record allocation/consumer evidence. Caller-filter code and factual documentation are pushed.\n\n'
for key in ('source_commit','data_commit','runtime_build','pdb','verification','native173','native174','native_limits','input_correction','selection_correction','next_task','unmet_full_goal','retired_xbox','daily_handoff'):
    value=checkpoint[key]
    intro+=f'{key}: '+('; '.join(value) if isinstance(value,list) else value)+'\n\n'
intro+='Both public worktrees clean, exact remote heads verified,53 preexisting/private renderer entries preserved. All native owned processes closed; no test is currently waiting. JOIN175 is planned only. No new protocol or live graphics repair claimed.\n\nEverything below is historical and superseded where it differs.\n\n'
progress.write_text(intro+progress.read_text(encoding='utf-8'),encoding='utf-8')
print(json.dumps({'at':now,'checkpoint':str(path),'source':source,'data':data,'goal':'active/incomplete'}))
