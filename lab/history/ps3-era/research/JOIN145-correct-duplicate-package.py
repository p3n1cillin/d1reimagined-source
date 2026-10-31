import json
from pathlib import Path
import sys

sys.path.insert(0, r'F:\d1\d1xenia\tools\content_capture')
from capture import Capture, atomic_json, copy_closed

root = Path(r'F:\d1\<pc>-import\d1xenia\local\content-capture')
capture = Capture(root / 'config.json')
clip = '<lab-pc>-20261006T082350-35a26ff986fc4d87a5428cdef765c812'
state_path = capture.states / (clip + '.json')
state = json.loads(state_path.read_text())
destination = Path(state['destination'])
expected_parent = Path(capture.cfg['CONTENT_NAS_PATH']) / 'incoming' / capture.machine / '2026' / '10' / '06'
assert destination.parent == expected_parent and destination.name == clip + '.ready'
local = capture.outbox / clip
hashes = capture.verify(local)
capture.verify(destination, hashes)
superseded = destination.with_suffix('.superseded')
assert not superseded.exists()
reason = 'A retried interrupted segment appeared twice; verified-segment deduplication now selects it once.'
audit = {'clip_id': clip, 'reason': reason, 'prior_destination': str(destination),
         'preserved_destination': str(superseded), 'prior_state': state,
         'original_local_and_NAS_hashes_verified': True}
atomic_json(root.parent / 'RESEARCH' / 'JOIN145-duplicate-package-correction.json', audit)
destination.rename(superseded)
capture.verify(superseded, hashes)
capture.state(clip, status='SUPERSEDED', destination=str(superseded), correction_reason=reason)
pending_path = root / 'pending-observations.json'
pending = json.loads(pending_path.read_text())
event = json.loads((root.parent / 'RESEARCH' / 'JOIN145-pending-before-timestamp-correction.json').read_text())[0]
assert event['event_id'] == 'e10992dda6414c7c90e3509c40f72888' and event['run'] == 'FINAL124_JOIN145'
acceptance = json.loads((root.parent / 'RESEARCH' / 'JOIN145-recording-acceptance.json').read_text())
event['detected_at'] = event['at']
event['at'] = acceptance['recording_observed_at']
event['supersedes_clip_id'] = clip
assert all(e['event_id'] != event['event_id'] for e in pending)
atomic_json(pending_path, pending + [event])
print(json.dumps({'preserved': str(superseded), 'replacement_queued': event['event_id']}))
