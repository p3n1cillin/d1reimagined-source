"""Read every reachable Git blob; report suspicious locations, never secret values."""
import argparse
import collections
import json
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('repo')
parser.add_argument('--out', required=True)
args = parser.parse_args()
repo = Path(args.repo).resolve()
def git(*argv):
    return subprocess.check_output(['git', '-C', str(repo), *argv])

objects = git('rev-list', '--objects', '--all').splitlines()
paths = {}
for item in objects:
    oid, _, name = item.partition(b' ')
    paths[oid.decode()] = name.decode('utf-8', 'replace')
patterns = {
    'private-key': re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----'),
    'github-token': re.compile(rb'\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})\b'),
    'aws-key': re.compile(rb'\b(?:AKIA|ASIA)[A-Z0-9]{16}\b'),
    'stripe-or-openai-key': re.compile(rb'\b(?:sk_live_[A-Za-z0-9]{20,}|sk-(?:proj-)?[A-Za-z0-9_-]{40,})\b'),
    'credential-url': re.compile(rb'https?://[^\s/:@]{1,100}:[^\s/@]{3,100}@'),
    'private-production': re.compile(rb'PRIVATE_ENGINEERING|handoff-policy\.md'),
}
findings = []
counts = collections.Counter()
blob_bytes = 0
largest = []
proc = subprocess.Popen(['git', '-C', str(repo), 'cat-file', '--batch'],
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE)
for oid, name in paths.items():
    proc.stdin.write((oid + '\n').encode()); proc.stdin.flush()
    header = proc.stdout.readline().decode().strip().split()
    if len(header) != 3: raise RuntimeError('Missing Git object')
    size = int(header[2]); data = proc.stdout.read(size)
    if len(data) != size or proc.stdout.read(1) != b'\n': raise RuntimeError('Truncated object')
    counts[header[1]] += 1
    if header[1] != 'blob': continue
    blob_bytes += size; largest.append((size, name))
    for category, pattern in patterns.items():
        for match in pattern.finditer(data):
            findings.append({'object': oid, 'path': name, 'category': category,
                             'line': data[:match.start()].count(b'\n') + 1})
proc.stdin.close()
if proc.wait() != 0: raise RuntimeError('Git object reader failed')
current_paths = git('ls-files', '-z').decode().split('\0')
suspect_paths = [p for p in current_paths if re.search(
    r'(^|/)(?:\.env(?:\.|$)|\.netrc$|id_rsa$|PRIVATE_ENGINEERING/|config\.local\.json$)', p)]
report = {'repo': str(repo), 'head': git('rev-parse', 'HEAD').decode().strip(),
          'counts': dict(counts), 'blob_bytes': blob_bytes, 'findings': findings,
          'suspect_current_paths': suspect_paths, 'largest_blobs': sorted(largest, reverse=True)[:12],
          'scope': 'All reachable refs and full blob bytes; targeted patterns, not proof of no secrets.'}
Path(args.out).write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
print(json.dumps({k: report[k] for k in ['head','counts','blob_bytes','findings','suspect_current_paths','largest_blobs']}))
raise SystemExit(2 if findings or suspect_paths else 0)
