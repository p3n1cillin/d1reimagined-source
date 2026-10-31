"""Use the configured GitHub credential without printing or persisting it."""
import json
import os
from pathlib import Path
import subprocess
import sys
from urllib.error import HTTPError
from urllib.request import Request, urlopen

repo = Path(r'F:\d1\public-work\d1reimagined-source')
private = Path(r'lab/history')
env = dict(os.environ, GIT_TERMINAL_PROMPT='0', GCM_INTERACTIVE='never')
r = subprocess.run(['git', '-C', str(repo), 'credential', 'fill'],
    input='url=https://github.com/p3n1cillin/d1reimagined-source.git\n\n',
    text=True, capture_output=True, env=env)
if r.returncode:
    raise SystemExit('GitHub credential unavailable noninteractively; no credential output exposed.')
credential = dict(line.split('=', 1) for line in r.stdout.splitlines() if '=' in line)
token = credential.get('password')
if not token:
    raise SystemExit('Configured GitHub credential contains no token.')

def api(path, body=None):
    req = Request('https://api.github.com/repos/p3n1cillin/d1reimagined-source/' + path,
        data=None if body is None else json.dumps(body).encode(),
        headers={'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
                 'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'd1-engineering'})
    try:
        with urlopen(req, timeout=30) as response:
            return json.load(response)
    except HTTPError as error:
        raise SystemExit(f'GitHub API request failed with HTTP {error.code}; no credentials exposed.') from None


if sys.argv[1] == '--branch-checks':
    checks = api('commits/' + sys.argv[2] + '/check-runs')
    print(json.dumps({'head': sys.argv[2], 'checks': [
        {'name': c['name'], 'status': c['status'], 'conclusion': c['conclusion']}
        for c in checks['check_runs']]}))
    raise SystemExit(0)

from urllib.parse import quote
if sys.argv[1] == '--status':
    number = int(sys.argv[2])
    pr = api(f'pulls/{number}')
    checks = api('commits/' + pr['head']['sha'] + '/check-runs')
    print(json.dumps({'number': number, 'state': pr['state'], 'merged': pr['merged'],
        'mergeable_state': pr.get('mergeable_state'), 'auto_merge': bool(pr.get('auto_merge')),
        'head': pr['head']['sha'], 'checks': [
            {'name': c['name'], 'status': c['status'], 'conclusion': c['conclusion']}
            for c in checks['check_runs']]}))
    raise SystemExit(0)
branch, title, body_file, receipt_file = sys.argv[1:]
existing = api('pulls?state=open&head=' + quote('p3n1cillin:' + branch, safe=''))
pr = existing[0] if existing else api('pulls', {
    'title': title, 'head': branch, 'base': 'main',
    'body': Path(body_file).read_text(), 'draft': False})
receipt = {'number': pr['number'], 'url': pr['html_url'], 'head': pr['head']['sha']}
Path(receipt_file).write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt))
