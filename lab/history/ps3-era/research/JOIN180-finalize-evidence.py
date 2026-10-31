import json,datetime,sys
from pathlib import Path
root=Path(__file__).parent;runtime=root.parent
def read(path): return json.loads(path.read_text(encoding='utf-8-sig'))
at=datetime.datetime.now(datetime.timezone.utc).isoformat()
closed=read(root/'JOIN180-independent-closed.json')
summary=read(root/'JOIN180-session-summary.json')
media=read(root/'JOIN180-video-inspection/receipt.json')
sys.path.insert(0,'F:/d1/d1xenia');sys.path.insert(0,'F:/d1/d1xenia/tools/content_capture')
from tools.content_capture.capture import validate_video
ffmpeg=runtime/'DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe'
unindexed=[]
for row in media['unindexed']:
    check={'segment':row['segment'],'original':row['source'],'original_present':row['original_present']}
    try:
        check['media']=validate_video(Path(row['source']),str(ffmpeg));check['full_decode']='PASS'
    except (ValueError,OSError) as exc:
        check['full_decode']='FAIL';check['error']=str(exc)
    unindexed.append(check)
result={'at':at,'run':'FINAL124_JOIN180','scope':'Ordinary same-player doorway/corridor input; no manual teleport24, no guest writes; incomplete streaming and presentation',
 'closed':closed,'session_summary':summary,'media_receipt':'JOIN180-video-inspection/receipt.json','unindexed_original_decode':unindexed,
 'indexed_sampled_views':'Model inspected one sample from each of the last two indexed recordings: frontend verification/loading only, not mission footage.',
 'route_result':read(root/'JOIN180-guardian-route/result.json'),
 'corridor_result':read(root/'JOIN180-natural-player-continuation-2/result.json'),
 'first_observer':read(root/'JOIN180-after-natural-entry/result.json'),
 'later_observer':read(root/'JOIN180-after-corridor/result.json'),
 'acceptance':{'movement':'PARTIAL','idle24_streaming':'FAIL','waypoints':'NOT ACCEPTED','full_revival_and_Ghost':'NOT ACCEPTED','combined_graphics':'FAIL','authentic_weapon_pickup':'NOT ACCEPTED','mission_completion':'NOT ACCEPTED'},
 'rsx_capture':'NOT ARMED/NOT SAVED','control_bytes':(runtime/'SIGNON_LOCAL/FINAL124_JOIN180-control.jsonl').stat().st_size}
assert closed['all_exact_owned_closed'] and closed['all_hashes_match']
(root/'JOIN180-final-acceptance.json').write_text(json.dumps(result,indent=2)+'\n')
review={'at':at,'model_reviewed_sheets':list(range(26)),'exact_seek_stills':305,'interval_seconds':5,
 'additional_cue_samples':True,'scope':'Full-duration visual sampling and closer revival/power/rifle samples, not every-frame playback or full audio review.',
 'source':read(root/'reference-mission1-20261008/source.json'),
 'correction':'Earlier selective review was overclaimed; user-reported missing markers, cutscene shots and Ghost remain accepted defects, not fixed.',
 'public_summary':'notes/guardian-rises-reference-review.md; original/stills remain private'}
(root/'reference-mission1-20261008/review.json').write_text(json.dumps(review,indent=2)+'\n')
print(json.dumps({'closed':True,'control_bytes':result['control_bytes'],'samples':summary['samples'],'unindexed_decode':unindexed,'reference_review_scope':review['scope']},indent=2))
