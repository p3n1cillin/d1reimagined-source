"""Documented source-only adaptations; never edit the private PS4 runtime."""
from pathlib import Path
import shutil
root = Path(r'F:\d1\public-work\d1reimagined-source')
p = root / 'ps4'
for name in ['d1-byte-scan.rs','d1-synthetic-gate.rs','Export-D1RepackSlack.ps1']:
    shutil.copyfile(Path(r'F:\d1\d1xenia\tools\ps4_setup')/name, root/'tools/ps4_setup'/name)
shutil.copyfile(Path(r'F:\d1\tools\synthetic-gate.rs'),
                p/'d1-server/crates/d1-run/src/gate_synthetic.rs')
gate = p/'d1-server/crates/d1-run/src/gate.rs'
s = gate.read_text(encoding='utf-8')
s = s.replace('include_bytes!("../testdata/gate-trace-excerpt.bin")', 'include!("gate_synthetic.rs")')
s = s.replace('A real trace excerpt (6 records, all probe 15, 4 with AL set), cut from run\n    /// 20260810-161501-RDOC-ORBIT-JANK by offline tooling.',
              'An original synthetic fixture (6 records, all probe 15, 4 with AL set),\n    /// generated through d1-trace make_record_bytes; no native input is included.')
s = s.replace('the real trace excerpt decoding', 'the synthetic fixture decoding')
gate.write_text(s, newline='\n', encoding='utf-8')
main = p/'d1-server/crates/d1/src/main.rs'
s = main.read_text(encoding='utf-8').replace('include_bytes!("../../d1-run/testdata/gate-trace-excerpt.bin")',
                           'include!("../../d1-run/src/gate_synthetic.rs")')
main.write_text(s, newline='\n', encoding='utf-8')
# Keep native golden tests as source, but require the excluded owned-input fixtures explicitly.
crate = p/'d1-server/crates/d1-eboot'
for name in ['catalogue.rs','ladder.rs']:
    path = crate/'tests'/name
    s = path.read_text(encoding='utf-8')
    path.write_text('#![cfg(feature = "private-fixtures")]\n'+s, newline='\n', encoding='utf-8')
with (crate/'Cargo.toml').open('a', newline='\n', encoding='utf-8') as f:
    f.write('\n[features]\n# Owned-input golden files are deliberately excluded from the public source.\nprivate-fixtures = []\n')
with (root/'.gitattributes').open('a', newline='\n', encoding='utf-8') as f:
    f.write('\nps4/** text eol=lf\nps4/**/*.patch -text\n')
with (root/'.gitignore').open('a', newline='\n', encoding='utf-8') as f:
    f.write('\n# Public PS4 source ships generators, never owned-input outputs.\n/ps4/dumps/\n/ps4/cfw-staging/\n/ps4/external/\n/ps4/historicaldocs/\n/ps4/runs/\n/ps4/captures/\n/ps4/catalogue/\n/ps4/manifest/\n**/target/\n*.pkg\n*.PUP\n*.elf\n*.self\n*.pem\n*.key\n.env\n.env.*\n/ps4/capture-rig/**/*.bin\n/ps4/capture-rig/**/*.tsv\n/ps4/ghidra/lists/\n/ps4/ghidra/structs/\n/ps4/d1-server/crates/d1-eboot/tests/fixtures/\n')
print('Applied public-only fixture and ignore adaptations')
