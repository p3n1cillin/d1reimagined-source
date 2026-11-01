"""The repository health report's rules (.github/scripts/health.py), on synthetic API records."""
import datetime as dt
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location('health', Path(__file__).parents[1] / '.github/scripts/health.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
NOW = dt.datetime(2026, 11, 10, tzinfo=dt.timezone.utc)


def stamp(days_ago):
    return (NOW - dt.timedelta(days=days_ago)).isoformat().replace('+00:00', 'Z')


def pr(login, days, labels=(), draft=False):
    return {'user': {'login': login}, 'updated_at': stamp(days), 'labels': [{'name': n} for n in labels], 'draft': draft}


def commit(login, name=None):
    return {'author': {'login': login} if login else None,
            'commit': {'author': {'name': name or login}, 'message': f'work by {name or login}'}}


class Health(unittest.TestCase):
    def test_a_contributors_pr_idle_over_a_day_is_waiting(self):
        prs = [pr('someone', 2), pr('someone', 0.5), pr('owner', 5), pr('someone', 5, ['approved']),
               pr('someone', 5, draft=True), pr('dependabot[bot]', 5)]
        self.assertEqual(M.waiting_prs(prs, 'owner', NOW), [prs[0]])

    def test_an_issue_without_a_maintainer_reply_after_two_days(self):
        old = {'number': 1, 'user': {'login': 'a'}, 'created_at': stamp(3), 'labels': []}
        answered = {'number': 2, 'user': {'login': 'a'}, 'created_at': stamp(3), 'labels': []}
        new = {'number': 3, 'user': {'login': 'a'}, 'created_at': stamp(1), 'labels': []}
        comments = {2: [{'user': {'login': 'owner'}}], 1: [{'user': {'login': 'b'}}]}
        self.assertEqual(M.unanswered([old, answered, new], comments, 'owner', NOW), [old])

    def test_only_commits_by_others_count_as_unshared_fork_work(self):
        # A fork copies our squash-merged branches: their commits are the maintainer's or Claude's.
        commits = [commit('owner'), commit(None, 'Claude'), commit('dependabot[bot]'), commit('ulf')]
        self.assertEqual(M.unshared(commits, 'owner'), [commits[3]])

    def test_a_commit_already_squash_merged_is_not_unshared(self):
        c = commit('ulf')
        c['commit']['message'] = 'notes: A Guardian Rises timeline from public videos\n\nbody'
        on_main = {M.subject('notes: A Guardian Rises timeline from public videos (#12)')}
        self.assertEqual(M.unshared([c], 'owner', on_main), [])
        self.assertEqual(M.subject('a (#x) b'), 'a (#x) b')


if __name__ == '__main__':
    unittest.main()
