# SPDX-License-Identifier: GPL-2.0-or-later
"""The full-game launch profiles (ps4/profiles) and their launcher (tools/ps4_setup/d1-full-game.ps1).

- Every knob full-game.env sets is still read somewhere: the servers' Rust code for [server], the
  script sidecar for [sidecar], and the docs (or a fork checkout in D1_FORK_DIR) for [emulator].
  A `?KEY` knob is code that is not merged or deployed yet; while its name is nowhere, it must be
  listed in docs/full-game-test.md's pending table instead. So a rename is caught.
- baseline.env turns off everything full-game.env turns on, and changes nothing else.
- d1-full-game.ps1 -DryRun parses (JSON), applies -Set and -Profile and -Only, and starts nothing.
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]
PROFILES = REPO / 'ps4' / 'profiles'
LAUNCHER = REPO / 'tools' / 'ps4_setup' / 'd1-full-game.ps1'
DOC = REPO / 'docs' / 'full-game-test.md'
SECTIONS = {'launch', 'server', 'sidecar', 'emulator'}
OFF = {'', '0'}
LINE = re.compile(r'^(\?)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$')


def read_profile(path, own_only=False, _stack=()):
    """The profile's knobs as {key: (section, value, optional, comment)}, includes first (as the
    launcher reads it). With own_only, only the lines in this file."""
    path = Path(path).resolve()
    if path in _stack:
        raise AssertionError(f'include cycle: {_stack + (path,)}')
    knobs, section, comment = {}, None, None
    for n, raw in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
        line = raw.strip()
        if not line:
            comment = None
            continue
        if line.startswith('#'):
            comment = line.lstrip('#').strip()
            continue
        m = re.match(r'^@include\s+(\S+)$', line)
        if m:
            if not own_only:
                knobs.update(read_profile(path.parent / m.group(1), False, _stack + (path,)))
            comment = None
            continue
        m = re.match(r'^\[(\w+)\]$', line)
        if m:
            assert m.group(1) in SECTIONS, f'{path.name}:{n}: unknown section {line}'
            section, comment = m.group(1), None
            continue
        m = LINE.match(line)
        assert m, f'{path.name}:{n}: not KEY=VALUE: {raw}'
        assert section, f'{path.name}:{n}: {m.group(2)} outside a section'
        knobs[m.group(2)] = (section, m.group(3), bool(m.group(1)), comment)
        comment = None
    return knobs


def corpus(globs, exclude=()):
    text = []
    for g in globs:
        for p in REPO.glob(g):
            if p.is_file() and not any(part in ('target', 'node_modules') for part in p.parts) and p not in exclude:
                text.append(p.read_text(encoding='utf-8', errors='replace'))
    return '\n'.join(text)


class ProfileFormat(unittest.TestCase):
    def test_every_knob_has_its_source_comment(self):
        for name in ('lab-base.env', 'full-game.env', 'baseline.env'):
            for key, (section, _, _, comment) in read_profile(PROFILES / name, own_only=True).items():
                with self.subTest(profile=name, knob=key):
                    self.assertTrue(comment, f'{name}: {key} needs one comment line above it naming its source')
                    if section != 'launch':
                        self.assertRegex(key, r'^D1_[A-Z0-9_]+$')

    def test_full_game_names_the_pull_request(self):
        for key, (section, _, _, comment) in read_profile(PROFILES / 'full-game.env', own_only=True).items():
            with self.subTest(knob=key):
                self.assertRegex(comment, r'#\d+|stream [A-Z]\d?', f'{key}: name the PR (#NNN, fork #N) or stream')

    def test_script_activity_lists_exist(self):
        table = PROFILES / 'script-activities.txt'
        for line in table.read_text(encoding='utf-8').splitlines():
            line = line.split('#')[0].strip()
            if line:
                fields = line.split()
                self.assertGreaterEqual(len(fields), 4, line)
                self.assertTrue((table.parent / fields[3]).is_file(), fields[3])


class KnobsExist(unittest.TestCase):
    """A knob the profile sets that nothing reads any more is a rename the profile missed."""

    @classmethod
    def setUpClass(cls):
        cls.rust = corpus(['ps4/d1-server/crates/**/*.rs'])
        cls.sidecar = corpus(['tools/script-host/**/*.mjs', 'tools/script-host/**/*.md'])
        own = {DOC, LAUNCHER, Path(__file__).resolve()}
        cls.docs = corpus(['docs/**/*.md', 'notes/**/*.md', 'ps4/**/*.md', 'tools/**/*.md', 'lab/**/*.ps1',
                           'tools/ps4_setup/*.ps1', 'tools/ps4_setup/*.py'], exclude=own)
        fork = os.environ.get('D1_FORK_DIR')
        cls.fork = ''
        if fork and Path(fork).is_dir():
            cls.fork = '\n'.join(p.read_text(encoding='utf-8', errors='replace')
                                 for p in list(Path(fork).glob('src/**/*.cpp')) + list(Path(fork).glob('d1/*.md')))
        cls.pending = DOC.read_text(encoding='utf-8') if DOC.is_file() else ''

    def where(self, section):
        return {'server': self.rust, 'sidecar': self.sidecar, 'emulator': self.docs + self.fork}[section]

    def test_every_full_game_knob_is_read_somewhere(self):
        for key, (section, _, optional, comment) in read_profile(PROFILES / 'full-game.env').items():
            if section == 'launch':
                continue
            with self.subTest(knob=key, section=section):
                if re.search(rf'\b{key}\b', self.where(section)):
                    continue
                # The sidecar's knobs pass through the host's environment, so they may name code that
                # is still to come (stream K's D1_BUNDLES_DIR); everything else must be a ?KEY.
                self.assertTrue(optional or section == 'sidecar',
                                f'{key} ([{section}], {comment}) is in no {section} code: renamed? '
                                'Fix the profile, or mark it ?KEY if its code is not merged yet')
                # Pending code must be accounted for in the doc's table (a row naming the knob).
                self.assertRegex(self.pending, rf'\|[^\n]*`{key}`[^\n]*\|',
                                 f'?{key} is not in the code yet; list it in docs/full-game-test.md')


class Baseline(unittest.TestCase):
    def test_baseline_turns_off_everything_full_game_turns_on(self):
        full_own = read_profile(PROFILES / 'full-game.env', own_only=True)
        base = read_profile(PROFILES / 'baseline.env')
        for key, (section, value, _, _) in full_own.items():
            with self.subTest(knob=key):
                self.assertIn(key, base, f'baseline.env must set {key} (off)')
                self.assertEqual(base[key][0], section)
                self.assertIn(base[key][1], OFF, f'baseline {key}={base[key][1]!r} is not off ("" or 0)')
                if value not in OFF:   # a knob full-game.env turns on (some are listed there as off)
                    self.assertNotEqual(base[key][1], value, f'{key} is the same in both profiles')

    def test_the_profiles_differ_only_in_the_new_knobs(self):
        full, base = read_profile(PROFILES / 'full-game.env'), read_profile(PROFILES / 'baseline.env')
        new = set(read_profile(PROFILES / 'full-game.env', own_only=True))
        self.assertEqual(set(read_profile(PROFILES / 'baseline.env', own_only=True)), new)
        for key in set(full) | set(base):
            if key not in new:
                self.assertEqual(full[key][1], base[key][1], key)


PWSH = shutil.which('pwsh')


@unittest.skipUnless(PWSH, 'pwsh is not installed')
class DryRun(unittest.TestCase):
    def run_dry(self, *args):
        with tempfile.TemporaryDirectory() as tmp:
            env = {k: v for k, v in os.environ.items() if not k.startswith('D1_') and k != 'CARGO_TARGET_DIR'}
            env.update(D1_ROOT=tmp, D1_PATHS_CONFIG=str(Path(tmp) / 'none.json'))
            r = subprocess.run([PWSH, '-NoProfile', '-NonInteractive', '-File', str(LAUNCHER), '-DryRun',
                                '-TargetDir', str(Path(tmp) / 'target'), *args],
                               capture_output=True, text=True, env=env, timeout=300)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertFalse(list(Path(tmp).iterdir()), 'a dry run must write nothing')
            return r.stdout

    def manifest(self, *args):
        return json.loads(self.run_dry('-Json', *args))

    def test_json_manifest(self):
        m = self.manifest('-Name', 'dry1', '-Set', 'D1_RSAT_NATIVE=0,server:D1_EXTRA_KNOB=5')
        knobs = {k['key']: k for k in m['knobs']}
        profile = read_profile(PROFILES / 'full-game.env')
        self.assertEqual(set(knobs), set(profile) | {'D1_EXTRA_KNOB'})
        self.assertEqual(knobs['D1_RSAT_NATIVE']['value'], '0')
        self.assertTrue(knobs['D1_RSAT_NATIVE']['override'])
        self.assertEqual(knobs['D1_EXTRA_KNOB']['section'], 'server')
        self.assertEqual(knobs['D1_FPS']['state'], 'unset')
        for key, (section, _, _, _) in profile.items():
            self.assertEqual(knobs[key]['section'], section, key)
        self.assertNotIn('${', json.dumps(m['knobs']), 'every placeholder resolves')
        self.assertIn('dry1', m['run'])
        self.assertIsInstance(m['ready'], bool)
        self.assertIn('Start-D1ContainedBoot.ps1', m['command'])
        self.assertIn('-OperatorInput', m['command'])
        self.assertIn('-ServerEnvFile', m['command'])
        self.assertTrue(m['dryRun'])
        self.assertEqual(len(m['chain']), 2)

    def test_baseline_profile(self):
        m = self.manifest('-Profile', 'baseline')
        knobs = {k['key']: k for k in m['knobs']}
        for key, (_, value, _, _) in read_profile(PROFILES / 'full-game.env', own_only=True).items():
            self.assertIn(knobs[key]['value'], OFF, key)
            if knobs[key]['value'] == '':
                self.assertEqual(knobs[key]['state'], 'unset', key)

    def test_only_modes(self):
        self.assertIn('-NoEmulator', self.manifest('-Only', 'servers')['command'])
        emulator = self.manifest('-Only', 'emulator')['command']
        self.assertIn('-NoServers', emulator)
        self.assertNotIn('-ServerBin', emulator)

    def test_human_output(self):
        out = self.run_dry('-Name', 'dry2')
        self.assertIn('[server]', out)
        self.assertIn('D1_STORE', out)
        self.assertRegex(out, r'(?m)^(READY|NOT READY)')

    def test_unknown_override_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            env = dict(os.environ, D1_ROOT=tmp, D1_PATHS_CONFIG=str(Path(tmp) / 'none.json'))
            r = subprocess.run([PWSH, '-NoProfile', '-NonInteractive', '-File', str(LAUNCHER), '-DryRun', '-Json',
                                '-Set', 'D1_NOT_IN_THE_PROFILE=1'], capture_output=True, text=True, env=env, timeout=300)
            self.assertNotEqual(r.returncode, 0)


class ContainedBoot(unittest.TestCase):
    def test_launcher_parameters_exist(self):
        text = (REPO / 'tools' / 'ps4_setup' / 'Start-D1ContainedBoot.ps1').read_text(encoding='utf-8')
        params = text[text.index('param('):text.index('$ErrorActionPreference')]
        for p in ('OutDir', 'ServerBin', 'ServerEnvFile', 'NoEmulator', 'NoServers', 'ExtraServerEnv', 'OperatorInput',
                  'ActivityHost', 'Activity', 'ClientPatch', 'LabIp', 'Minutes', 'Name'):
            self.assertRegex(params, rf'\]\${p}\b', p)
        launcher = LAUNCHER.read_text(encoding='utf-8')
        for flag in re.findall(r"'-([A-Z][A-Za-z]+)'", launcher[launcher.index('$bootArgs = @('):launcher.index('$bootLine')]):
            if flag not in ('NoProfile', 'File'):
                self.assertRegex(params, rf'\]\${flag}\b', flag)


if __name__ == '__main__':
    unittest.main()
