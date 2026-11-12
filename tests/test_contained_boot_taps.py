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
        m = re.search(r'^function Resolve-D1EnvInt\b.*?^}', cls.text, re.S | re.M)
        assert m, 'Resolve-D1EnvInt not found'
        cls.resolve_func = m.group(0)

    def test_parses(self):
        r = ps("$e = $null; [void][System.Management.Automation.Language.Parser]::ParseFile("
               f"'{SCRIPT}', [ref]$null, [ref]$e); $e.Count")
        self.assertEqual(r.stdout.strip(), '0', r.stdout + r.stderr)

    def state(self, chunk, **before):
        start = {'Title': False, 'Wizard': False, 'Character': False, 'Start': False}
        start.update(before)
        pre = ';'.join(f'${k} = ${str(v).lower()}' for k, v in start.items())
        chunk_file = Path(__file__).with_suffix('.chunk.tmp')
        chunk_file.write_text(chunk, encoding='utf-8')
        try:
            cmd = (f"{self.func}\n{pre}\n$t = [IO.File]::ReadAllText('{chunk_file}')\n"
                   "$r = Get-D1BootState $t @{ Title = $Title; Wizard = $Wizard; Character = $Character; Start = $Start }\n"
                   "[pscustomobject]@{ Title = $r.Title; Wizard = $r.Wizard; Character = $r.Character; Start = $r.Start; "
                   "LeftWizard = $r.LeftWizard } | ConvertTo-Json -Compress")
            r = ps(cmd)
        finally:
            chunk_file.unlink(missing_ok=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def test_entering_only(self):
        for key, name in (('Title', 'bootflow:intro_screen_display'), ('Wizard', 'bootflow:profile_setup'),
                          ('Character', 'character:signin'), ('Start', 'bootflow:start')):
            s = self.state(line('Entering', name))
            self.assertTrue(s[key], name)
            self.assertEqual(sum(1 for k in ('Title', 'Wizard', 'Character', 'Start') if s[k]), 1, name)

    def test_entering_then_leaving_in_one_chunk(self):
        for key, name in (('Title', 'bootflow:intro_screen_display'), ('Wizard', 'bootflow:profile_setup'),
                          ('Character', 'character:signin'), ('Start', 'bootflow:start')):
            self.assertFalse(self.state(line('Entering', name) + line('Leaving', name))[key], name)

    def test_leaving_then_entering_in_one_chunk(self):
        for key, name in (('Title', 'bootflow:intro_screen_display'), ('Wizard', 'bootflow:profile_setup'),
                          ('Character', 'character:signin'), ('Start', 'bootflow:start')):
            self.assertTrue(self.state(line('Leaving', name) + line('Entering', name))[key], name)

    def test_leaving_clears_and_unrelated_text_keeps(self):
        s = self.state(line('Leaving', 'character:signin'), Character=True)
        self.assertFalse(s['Character'])
        s = self.state('Loaded 12 shaders\nnothing to see\n', Character=True, Wizard=True)
        self.assertTrue(s['Character'] and s['Wizard'])
        self.assertFalse(s['Title'] or s['LeftWizard'])

    def test_intro_leaving_and_start_entering_in_one_chunk(self):
        chunk = line('Leaving', 'bootflow:intro_screen_display') + line('Entering', 'bootflow:start')
        s = self.state(chunk, Title=True)
        self.assertTrue(s['Start'])
        self.assertFalse(s['Title'])

    def test_start_leaving_clears(self):
        chunk = line('Leaving', 'bootflow:start') + line('Entering', 'bootflow:platform_signin')
        self.assertFalse(self.state(chunk, Start=True)['Start'])

    def test_left_wizard_flag(self):
        self.assertTrue(self.state(line('Leaving', 'bootflow:profile_setup'), Wizard=True)['LeftWizard'])

    def resolve(self, default, passed, env):
        cmd = (f"{self.resolve_func}\n$r = Resolve-D1EnvInt {default} ${str(passed).lower()} 'D1_X' '{env}'\n"
               "[pscustomobject]@{ Value = $r.Value; Note = $r.Note } | ConvertTo-Json -Compress")
        r = ps(cmd)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def test_env_override(self):
        self.assertEqual(self.resolve(3, False, '')['Value'], 3)
        self.assertEqual(self.resolve(3, False, '0')['Value'], 0)
        self.assertEqual(self.resolve(20, False, '45')['Value'], 45)
        self.assertIsNone(self.resolve(3, False, '5')['Note'])

    def test_env_override_loses_to_the_parameter(self):
        self.assertEqual(self.resolve(3, True, '0')['Value'], 3)

    def test_env_override_ignores_a_non_integer(self):
        for bad in ('abc', '1.5', '-2'):
            r = self.resolve(3, False, bad)
            self.assertEqual(r['Value'], 3, bad)
            self.assertIn('ignored D1_X', r['Note'], bad)

    def test_script_wires_the_override_and_the_truncation_reset(self):
        self.assertIn("$PSBoundParameters.ContainsKey('CharacterTaps')", self.text)
        self.assertIn('D1_BOOT_CHARACTER_TAPS', self.text)
        self.assertIn('D1_BOOT_CHARACTER_GRACE', self.text)
        self.assertIn("$PSBoundParameters.ContainsKey('StartTaps')", self.text)
        self.assertIn('D1_BOOT_START_TAPS', self.text)
        self.assertIn('D1_BOOT_START_GRACE', self.text)
        self.assertRegex(self.text, r'\$logPos = 0L; \$atTitle = \$false; \$inWizard = \$false; \$atCharacter = \$false; \$atStart = \$false')

    def test_parameters_and_receipt(self):
        self.assertRegex(self.text, r'\[int\]\$CharacterTaps = 3')
        self.assertRegex(self.text, r'\[int\]\$CharacterGraceSeconds = 20')
        self.assertIn('character taps every ${TapIntervalSeconds}s', self.text)
        self.assertRegex(self.text, r'\[int\]\$StartTaps = 6')
        self.assertRegex(self.text, r'\[int\]\$StartGraceSeconds = 5')
        self.assertIn('start taps every ${TapIntervalSeconds}s once bootflow:start', self.text)
        self.assertIn('$receipt.startTaps = $startTapCount', self.text)


if __name__ == '__main__':
    unittest.main()
