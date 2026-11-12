# SPDX-License-Identifier: GPL-2.0-or-later
"""Start-D1ContainedBoot.ps1 taps by the game's own state line (offline: the emulator is never started).

- The script parses.
- Get-D1BootState, the pure function the tap loop calls, is extracted and run against log chunks:
  Entering only, Entering then Leaving in one chunk, Leaving then Entering in one chunk, unrelated text.
- The character tap parameters exist and are recorded in the receipt.
"""
import json
from pathlib import Path
import re
import shutil
import subprocess
import unittest

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / 'tools' / 'ps4_setup' / 'Start-D1ContainedBoot.ps1'
PWSH = shutil.which('pwsh') or shutil.which('powershell')


def ps(command):
    return subprocess.run([PWSH, '-NoProfile', '-NonInteractive', '-Command', command],
                          capture_output=True, text=True, timeout=120)


def line(verb, state):
    return f"[Lib.Core] <Info> world_controller:state_manager: {verb} state '{state}' for reason 'normal_flow'.\n"


@unittest.skipUnless(PWSH, 'neither pwsh nor powershell is on PATH')
class ContainedBootTaps(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = SCRIPT.read_text(encoding='utf-8')
        m = re.search(r'^function Get-D1BootState\b.*?^}', cls.text, re.S | re.M)
        assert m, 'Get-D1BootState not found'
        cls.func = m.group(0)

    def test_parses(self):
        r = ps("$e = $null; [void][System.Management.Automation.Language.Parser]::ParseFile("
               f"'{SCRIPT}', [ref]$null, [ref]$e); $e.Count")
        self.assertEqual(r.stdout.strip(), '0', r.stdout + r.stderr)

    def state(self, chunk, **before):
        start = {'Title': False, 'Wizard': False, 'Character': False}
        start.update(before)
        pre = ';'.join(f'${k} = ${str(v).lower()}' for k, v in start.items())
        chunk_file = Path(__file__).with_suffix('.chunk.tmp')
        chunk_file.write_text(chunk, encoding='utf-8')
        try:
            cmd = (f"{self.func}\n{pre}\n$t = [IO.File]::ReadAllText('{chunk_file}')\n"
                   "$r = Get-D1BootState $t @{ Title = $Title; Wizard = $Wizard; Character = $Character }\n"
                   "[pscustomobject]@{ Title = $r.Title; Wizard = $r.Wizard; Character = $r.Character; "
                   "LeftWizard = $r.LeftWizard } | ConvertTo-Json -Compress")
            r = ps(cmd)
        finally:
            chunk_file.unlink(missing_ok=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def test_entering_only(self):
        for key, name in (('Title', 'bootflow:intro_screen_display'), ('Wizard', 'bootflow:profile_setup'),
                          ('Character', 'character:signin')):
            s = self.state(line('Entering', name))
            self.assertTrue(s[key], name)
            self.assertEqual(sum(1 for k in ('Title', 'Wizard', 'Character') if s[k]), 1, name)

    def test_entering_then_leaving_in_one_chunk(self):
        for key, name in (('Title', 'bootflow:intro_screen_display'), ('Wizard', 'bootflow:profile_setup'),
                          ('Character', 'character:signin')):
            self.assertFalse(self.state(line('Entering', name) + line('Leaving', name))[key], name)

    def test_leaving_then_entering_in_one_chunk(self):
        for key, name in (('Title', 'bootflow:intro_screen_display'), ('Wizard', 'bootflow:profile_setup'),
                          ('Character', 'character:signin')):
            self.assertTrue(self.state(line('Leaving', name) + line('Entering', name))[key], name)

    def test_leaving_clears_and_unrelated_text_keeps(self):
        s = self.state(line('Leaving', 'character:signin'), Character=True)
        self.assertFalse(s['Character'])
        s = self.state('Loaded 12 shaders\nnothing to see\n', Character=True, Wizard=True)
        self.assertTrue(s['Character'] and s['Wizard'])
        self.assertFalse(s['Title'] or s['LeftWizard'])

    def test_left_wizard_flag(self):
        self.assertTrue(self.state(line('Leaving', 'bootflow:profile_setup'), Wizard=True)['LeftWizard'])

    def test_parameters_and_receipt(self):
        self.assertRegex(self.text, r'\[int\]\$CharacterTaps = 3')
        self.assertRegex(self.text, r'\[int\]\$CharacterGraceSeconds = 20')
        self.assertIn('character taps every ${TapIntervalSeconds}s', self.text)


if __name__ == '__main__':
    unittest.main()
