"""Private daily evidence preparation; preserves untouched native originals."""
import hashlib
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO = Path('F:/d1/d1xenia')
RT = Path('F:/d1/<pc>-import/d1xenia/local')
WORK = Path(__file__).parent
sys.path.insert(0, str(REPO / 'tools/content_capture'))
from capture import Capture

capture = Capture(RT / 'content-capture/config.json')
specs = [
    dict(run='JOIN252', file='BLES01857_recording_2026_10_09_22_18_25.mp4',
         recorded_at='2026-10-09T21:18:25+00:00', feature='urgent-dialog-suppression',
         description='The Khvostov pickup notification appears without the unwanted urgent dialog in this bounded native run.',
         previous_state='An unsolicited urgent dialog interrupted input immediately after the rifle grant.',
         current_state='Sampled frames around the grant show the Khvostov notification and rifle with no urgent dialog; the trace records zero dialog enqueues.',
         remaining_issue='The NEW MISSION banner still appears in this run; the scene remains dark, the early Level Up coach remains and the rifle magazine reads 12.',
         tested_source_base='bb55871b71620a0800f94f5f6cc4a7b6818e20aa',
         tested_uncommitted_changes='Launch universe flags 2059/2065/2066 set false, later committed in 48c3992; documentation changes also present.',
         useful_timecodes='0-15 s: approach, acquisition notification and first-person rifle without urgent dialog; 20-27 s: unwanted NEW MISSION banner still visible.'),
    dict(run='JOIN253', file='BLES01857_recording_2026_10_10_01_53_41.mp4',
         recorded_at='2026-10-10T00:53:41+00:00', feature='objective-banner-order',
         description='The Wall objective changes from Enter the Wall to Find a Weapon without the unwanted NEW MISSION banner in this sampled native handover.',
         previous_state='Starting an objective in incoming state raised NEW MISSION / The Taken King: Prelude.',
         current_state='The sampled clip shows Enter the Wall and then Find a Weapon, with no NEW MISSION banner at this handover.',
         remaining_issue='A later activity connection loss causes a rejoin; no post-grant banner acceptance in this run. Darkness, early Level Up coach, kills and correct ammunition remain open.',
         tested_source_base='48c39925f5b15324747c39239d76ea851c9945a4',
         tested_uncommitted_changes='Mission objectives initially active (state 5), later committed in 1d5b7e2.',
         useful_timecodes='5-12 s: Enter the Wall; 14-25 s: Find a Weapon, no NEW MISSION banner in these sampled frames.'),
]
results = []
for spec in specs:
    run = spec['run']
    source = RT / 'RPCS3_FINAL124_SMOKE001/recordings/BLES01857' / spec['file']
    assert source.is_file() and source.stat().st_size > 0
    native = RT / f'SIGNON_LOCAL/logs/{run}-pre-stop-native.log'
    assert spec['file'] in native.read_text(encoding='utf-8', errors='replace')
    closure = json.loads((RT / f'RESEARCH/{run}-independent-closure-verified.json').read_text(encoding='utf-8-sig'))
    assert closure['all_hashes_match'] and not any(c['exact_owned_alive'] for c in closure['checks'])
    metadata = {k: v for k, v in spec.items() if k != 'file'}
    metadata.update(
        reporting_date='2026-10-10', reporting_timezone='Europe/London',
        daily_update_id='destiny1-2026-10-10', development_session='opening-mission-ui-order',
        topic='opening-mission', event_type='FEATURE_WORKING', importance='USEFUL',
        game_build='BLES01857 01.24; patched RPCS3 C5',
        tested_worktree_clean=False, derivative=False, source='Untouched owned native RPCS3 game/audio recording segment',
        maturity='Partially Working; sampled and bounded',
        suggested_use=['private-production-review', 'invited-member-update-after-review'],
        contains_voice=False, microphone='Not captured by the native recorder',
        overlay_time_ranges=['0-5 s: recording-start/save notices include a recording path; inspect and exclude or redact before release.',
                             'Whole clip: out-of-order in-game Level Up coach.'],
        capture_notes='Contact sheets sampled about once every three seconds; not an inspection of every video frame. Full decoding is performed by the existing packager.',
        music_note='Original game audio retained; music content not audited or rights-cleared.',
        rights_note='Destiny game content; private evidence only, downstream privacy and rights review required.',
        automatic_security_check={'target': 'owned RPCS3 native game and audio only',
                                  'text_scan': 'PASS', 'visual_scan': 'sampled only; complete publication review still required',
                                  'publication_review_required': True},
    )
    package = capture.package(source, metadata, reviewed=False)
    capture.state(package.name, status='WAITING_FOR_NAS', automatic_game_source=True)
    meta = json.loads((package / 'metadata.json').read_text())
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    assert meta['sha256'] == source_hash and meta['full_decode'] == 'PASS'
    results.append({'run': run, 'package_id': package.name, 'source_file': str(source),
                    'recorded_at': spec['recorded_at'], 'source_sha256': source_hash,
                    'package_path': str(package), 'duration_seconds': meta['duration_seconds'],
                    'video_properties': meta['video_properties'], 'contains_audio': meta['contains_audio'],
                    'full_decode': meta['full_decode'], 'requires_manual_review': meta['requires_manual_review']})
    print(json.dumps(results[-1]), flush=True)
out = WORK / 'new-evidence-packages.json'
assert not out.exists()
out.write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')
