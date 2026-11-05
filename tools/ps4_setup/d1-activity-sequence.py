# SPDX-License-Identifier: GPL-2.0-or-later
"""Describe a live-server capture's activity as an encounter sequence the activity framework runs.

    d1-activity-sequence.py CAPTURE.pcap --activity-id ID --name NAME [--out FILE.json]

Reads one SRCP capture (notes/d1-live-captures-2026-11-05.md) and writes, for its first activity
host (the PAH, which stays connected all mission), the stages the player went through:

- a stage begins the first time the client reports a slice set it has not reported before
  (udt 21 +0xB8, the region number `bubble << 3 | state`);
- each stage lists the sensor groups the PAH first served during it (udt 5), the cinematics that
  started and stopped (51539 incidents 2120 and 734), the triggers the player crossed (51539
  incident 2665, decoded as `trigger_report` does) and the extra activity hosts that opened
  (public instances);
- the sequence ends at the PAH's last client message.

Only findings leave the capture: times, slice sets, group hashes, trigger names and counts. No
message body, no account data, no player name. The output is the framework's `CaptureSequence`
shape (`d1_activity_host::encounter_framework`), labelled `evidence: live-capture` with the build
(51539, Xbox 360) it was recorded on, which is not ours: incident ids are numbered per build and
the timings are one solo player's.

Decoding uses the 56578 client's codec through `d1-oracle` (D1_EBOOT, D1_ORACLE), as d1-srcp.py.
"""
import argparse
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import d1_srcp  # noqa: E402

_spec = importlib.util.spec_from_file_location('d1srcp_cli', HERE / 'd1-srcp.py')
srcp = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(srcp)

# 51539 (Xbox 360) client incident ids; 56578's are +157, +561, +738 (the live-captures note).
CINE_STOPPED, CINE_STARTED, TRIGGER_CROSSED = 734, 2120, 2665
SLICE_SET_OFFSET = 0xB8


def bits(body, at, n):
    v = 0
    for k in range(at, at + n):
        if k // 8 >= len(body):
            return None
        v = (v << 1) | ((body[k // 8] >> (7 - k % 8)) & 1)
    return v


def trigger_name(words):
    """`group/kind/index` of a trigger report (d1-activity-host `trigger_report`), or None."""
    body = srcp.image(words)[0xD8:]
    h, kind, index = bits(body, 303, 32), bits(body, 335, 7), bits(body, 342, 16)
    if None in (h, kind, index) or h in (0, 0x811C9DC5) or kind < 1 or index < 0x8000:
        return None
    return f'{h:#010x}/{kind - 1}/{index - 0x8000}'


def sequence(cap, activity_id, name):
    data = cap.read_bytes()
    sess = d1_srcp.sessions(data)
    if not sess:
        sys.exit(f'{cap.name}: no activity-host session')
    start, pah, msgs = sess[0]
    pick = [(u, b) for _, d, _, u, b in msgs if (d, u) in (('c2s', 21), ('c2s', 18))]
    words = srcp.decode_all(pick)
    frames = srcp.sensor_frames(b for _, d, _, u, b in msgs if d == 's2c' and u == srcp.SENSOR)

    stages, seen_slices, seen_groups, markers = [], set(), set(), []
    pending_groups = []  # groups served before the first slice report

    def stage():
        return stages[-1] if stages else None

    for t, d, _, u, b in msgs:
        rel = round(t - start, 1)
        if d == 'c2s' and u == 21:
            w = words.get((u, b))
            if w and SLICE_SET_OFFSET in w:
                s = w[SLICE_SET_OFFSET] & 0x3FF
                if s not in (0, 0x3FF) and s not in seen_slices:
                    seen_slices.add(s)
                    markers.append({'t_s': rel, 'slice': s})
                    stages.append({'slice_set': s, 'entered_s': rel, 'groups_first_served': pending_groups,
                                   'cinematics_started': 0, 'cinematics_stopped': 0, 'triggers': [],
                                   'public_hosts_opened': 0})
                    pending_groups = []
        elif d == 's2c' and u == srcp.SENSOR:
            f = frames.get(b)
            for e in (f or {}).get('entries', []):
                g = e['hash']
                if g not in seen_groups:
                    seen_groups.add(g)
                    (stage()['groups_first_served'] if stage() else pending_groups).append(g)
        elif d == 'c2s' and u == 18 and stage():
            w = words.get((u, b))
            if not w:
                continue
            types = srcp.incident_types(w)
            st = stage()
            st['cinematics_started'] += types.count(CINE_STARTED)
            st['cinematics_stopped'] += types.count(CINE_STOPPED)
            for ty, label in ((CINE_STARTED, 'cinematic_started'), (CINE_STOPPED, 'cinematic_stopped')):
                if ty in types:
                    markers.append({'t_s': rel, label: True})
            if TRIGGER_CROSSED in types:
                tr = trigger_name(w)
                if tr and tr not in st['triggers']:
                    st['triggers'].append(tr)
                    markers.append({'t_s': rel, 'trigger': tr})
    for s2, _, _ in sess[1:]:
        for st in reversed(stages):
            if st['entered_s'] <= round(s2 - start, 1):
                st['public_hosts_opened'] += 1
                break
    end = round(msgs[-1][0] - start, 1)
    for a, b in zip(stages, stages[1:] + [None]):
        a['duration_s'] = round((b['entered_s'] if b else end) - a['entered_s'], 1)
    return {
        'activity_id': activity_id,
        'name': name,
        'evidence': {'kind': 'live-capture', 'build': '51539 (Xbox 360)', 'capture': cap.stem[20:],
                     'note': 'one solo run (Crota\'s End: a fireteam); incident ids are 51539\'s; '
                             'see notes/d1-live-captures-2026-11-05.md'},
        'host_session_s': end,
        'hosts': len(sess),
        'stages': stages,
        'markers': markers,
        'segments': segments(markers, end),
    }


# A gap of this long with no new trigger or slice is read as a fight: the next gate opens a new
# segment. A heuristic over one run, stated as such in every segment it makes.
FIGHT_GAP_S = 45.0


def segments(markers, end):
    """Encounter segments: each opens at a gate (a first slice entry, or the first trigger after
    FIGHT_GAP_S of quiet) and runs to the next gate. `begins_on` is what the framework waits for."""
    gates, last = [], None
    for m in markers:
        if 'slice' in m:
            gates.append(m)
        elif 'trigger' in m and (last is None or m['t_s'] - last >= FIGHT_GAP_S):
            gates.append(m)
        if 'slice' in m or 'trigger' in m:
            last = m['t_s']
    out = []
    for i, (g, nxt) in enumerate(zip(gates, gates[1:] + [None])):
        begins = {'slice_entered': g['slice']} if 'slice' in g else {'trigger': g['trigger']}
        out.append({'index': i, 'begins_on': begins, 'at_s': g['t_s'],
                    'duration_s': round((nxt['t_s'] if nxt else end) - g['t_s'], 1)})
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('capture', type=Path)
    ap.add_argument('--activity-id', required=True)
    ap.add_argument('--name', required=True)
    ap.add_argument('--out', type=Path)
    a = ap.parse_args()
    seq = sequence(a.capture, a.activity_id, a.name)
    text = json.dumps(seq, indent=2) + '\n'
    if a.out:
        a.out.write_text(text, encoding='utf-8')
        print(f'{a.capture.stem[20:]}: {len(seq["stages"])} stages -> {a.out}')
    else:
        sys.stdout.write(text)


if __name__ == '__main__':
    main()
