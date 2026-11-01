"""The repository health report: one pinned issue, rewritten each day by the sweep workflow.

    GITHUB_TOKEN=... GITHUB_REPOSITORY=owner/repo python .github/scripts/health.py [--dry-run]

It lists what goes stale when nobody looks:
- contributors' pull requests waiting more than a day for a maintainer;
- issues with no maintainer reply after two days;
- how many `good first issue` tasks are open (the target is at least 5);
- how old docs/STATUS.md is;
- work in forks: commits ahead of main, authored by someone other than the maintainer, on a branch
  with no pull request. A fork copies every branch, and our squash-merged branches look "ahead"
  in it, but their commits are the maintainer's, so they don't count. Nor does a commit whose
  subject is already on main (a squash merge keeps the PR title, usually the commit's subject);

- branches here whose pull requests have merged.

It only reads, apart from writing that one issue (labelled `maintenance`). `--dry-run` prints the body.
"""
import argparse
import datetime as dt
import json
import os
import sys
import urllib.parse
import urllib.request

TITLE = 'Repository health'
LABEL = 'maintenance'
BOTS = ('[bot]',)


def api(path, token, method='GET', body=None):
    url = path if path.startswith('https://') else f'https://api.github.com/{path}'
    req = urllib.request.Request(url, method=method, data=json.dumps(body).encode() if body is not None else None,
                                 headers={'Authorization': f'Bearer {token}', 'Accept': 'application/vnd.github+json'})
    with urllib.request.urlopen(req) as r:
        return json.load(r) if r.status != 204 else None


def pages(path, token, limit=10):
    out = []
    for page in range(1, limit + 1):
        sep = '&' if '?' in path else '?'
        got = api(f'{path}{sep}per_page=100&page={page}', token)
        out += got
        if len(got) < 100:
            break
    return out


def age_days(stamp, now):
    return (now - dt.datetime.fromisoformat(stamp.replace('Z', '+00:00'))).total_seconds() / 86400


def waiting_prs(prs, owner, now):
    """Contributors' ready PRs whose last update is over a day old and that aren't approved."""
    out = []
    for p in prs:
        labels = {l['name'] for l in p['labels']}
        if p['user']['login'] == owner or p['user']['login'].endswith(BOTS) or p['draft']:
            continue
        if labels & {'approved', 'do not merge'}:
            continue
        if age_days(p['updated_at'], now) > 1:
            out.append(p)
    return out


def unanswered(issues, comments_by_issue, owner, now):
    """Open issues over two days old that the maintainer has neither written nor replied to."""
    out = []
    for i in issues:
        if 'pull_request' in i or i['user']['login'] == owner or LABEL in {l['name'] for l in i['labels']}:
            continue
        if age_days(i['created_at'], now) > 2 and not any(c['user']['login'] == owner
                                                          for c in comments_by_issue.get(i['number'], [])):
            out.append(i)
    return out


def subject(message):
    """A commit's first line without a squash merge's ` (#123)`."""
    first = message.splitlines()[0].strip() if message else ''
    head, _, tail = first.rpartition(' (#')
    return head if head and tail.endswith(')') and tail[:-1].isdigit() else first


def unshared(ahead_commits, owner, on_main=frozenset()):
    """Commits ahead of main that someone other than the maintainer (or a bot) wrote, and whose
    subject isn't already on main."""
    who = lambda c: (c.get('author') or {}).get('login') or c['commit']['author']['name']
    return [c for c in ahead_commits if who(c) != owner and not who(c).endswith(BOTS) and who(c) != 'Claude'
            and subject(c['commit']['message']) not in on_main]


def report(repo, token, now):
    owner = repo.split('/')[0]
    lines = [f'Updated {now:%Y-%m-%d %H:%M} UTC by the daily sweep. Each section is empty when all is well.', '']

    prs = pages(f'repos/{repo}/pulls?state=open', token)
    wait = waiting_prs(prs, owner, now)
    lines += ['### Pull requests waiting on a maintainer', '']
    lines += [f'- [ ] #{p["number"]} by @{p["user"]["login"]}, idle {age_days(p["updated_at"], now):.0f} days' for p in wait] or ['None.']

    issues = [i for i in pages(f'repos/{repo}/issues?state=open', token) if 'pull_request' not in i]
    comments = {i['number']: pages(i['comments_url'], token, 2) for i in issues if i['comments']}
    lines += ['', '### Issues with no maintainer reply after two days', '']
    lines += [f'- [ ] #{i["number"]} {i["title"]}' for i in unanswered(issues, comments, owner, now)] or ['None.']

    gfi = [i for i in issues if 'good first issue' in {l['name'] for l in i['labels']}]
    lines += ['', '### Starter tasks', '', f'{len(gfi)} open `good first issue` tasks' +
              (' (fewer than 5: write more from real gaps).' if len(gfi) < 5 else '.')]

    commits = api(f'repos/{repo}/commits?path=docs/STATUS.md&per_page=1', token)
    if commits:
        days = age_days(commits[0]['commit']['committer']['date'], now)
        lines += ['', '### Contributor checkpoint', '',
                  f'docs/STATUS.md last changed {days:.0f} days ago' + (' (update it).' if days > 3 else '.')]

    lines += ['', '### Work in forks without a pull request', '']
    heads = {f'{(p["head"]["repo"] or {}).get("full_name")}:{p["head"]["ref"]}' for p in pages(f'repos/{repo}/pulls?state=all', token, 3)}
    found = []
    on_main = {subject(c['commit']['message']) for c in pages(f'repos/{repo}/commits?sha=main', token, 10)}
    for fork in pages(f'repos/{repo}/forks', token, 2):
        if age_days(fork['pushed_at'], now) > 60:
            continue
        for b in pages(f'repos/{fork["full_name"]}/branches', token, 2):
            if f'{fork["full_name"]}:{b["name"]}' in heads:
                continue
            ref = urllib.parse.quote(f'{fork["owner"]["login"]}:{fork["name"]}:{b["name"]}', safe=':')
            try:
                cmp = api(f'repos/{repo}/compare/main...{ref}', token)
            except Exception:
                continue
            mine = unshared(cmp.get('commits', []), owner, on_main)
            if mine:
                found.append(f'- [ ] [{fork["full_name"]}@{b["name"]}]({fork["html_url"]}/tree/{urllib.parse.quote(b["name"])}): '
                             f'{len(mine)} commit(s), latest "{mine[-1]["commit"]["message"].splitlines()[0][:80]}"')
    lines += found or ['None.']
    lines += ['', 'Invite the author to open a (draft) pull request: '
              '[CONTRIBUTING.md](../blob/main/CONTRIBUTING.md#already-working-in-a-fork-or-your-own-project).']

    merged = {p['head']['ref'] for p in pages(f'repos/{repo}/pulls?state=closed', token, 3)
              if p['merged_at'] and (p['head']['repo'] or {}).get('full_name') == repo}
    left = [b['name'] for b in pages(f'repos/{repo}/branches', token, 3) if b['name'] in merged]
    lines += ['', '### Merged branches still here', '',
              f'{len(left)} (their pull requests merged; each can be restored from its PR page).' if left else 'None.']
    return '\n'.join(lines) + '\n'


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--dry-run', action='store_true')
    a = ap.parse_args()
    token, repo = os.environ['GITHUB_TOKEN'], os.environ['GITHUB_REPOSITORY']
    body = report(repo, token, dt.datetime.now(dt.timezone.utc))
    if a.dry_run:
        print(body)
        return 0
    found = [i for i in api(f'repos/{repo}/issues?state=open&labels={LABEL}&per_page=100', token) if i['title'] == TITLE]
    if found:
        api(f'repos/{repo}/issues/{found[0]["number"]}', token, 'PATCH', {'body': body})
    else:
        api(f'repos/{repo}/issues', token, 'POST', {'title': TITLE, 'body': body, 'labels': [LABEL]})
    return 0


if __name__ == '__main__':
    sys.exit(main())
