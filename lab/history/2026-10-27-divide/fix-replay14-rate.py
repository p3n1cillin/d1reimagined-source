"""Private fixed-rate discriminator on the existing driver, with fresh post-death tracking."""
from pathlib import Path
p = Path(__file__).resolve().parent
source = Path(r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup')
fixed = p/'fixed-rate-driver'
fixed.mkdir(exist_ok=False)
for name in ['d1-winguest.py','d1-step.py']:
    (fixed/name).write_bytes((source/name).read_bytes())
text = (source/'d1-walk.py').read_text()
assert 'rate, sign = 0.0035' in text
text = text.replace('rate, sign = 0.0035','rate, sign = 0.00226')
text = text.replace('heading, calibrated = None, False','heading, calibrated = None, True')
(fixed/'d1-walk.py').write_text(text)
file = p/'replay-divide14.py'
text = file.read_text().replace("TOOLS = REPO / 'tools/ps4_setup'",f'TOOLS = Path({str(fixed)!r})')
text = text.replace("'enter-wall-tolerance25'","'enter-wall-fixed-rate'")
file.write_text(text)
print('Private driver uses previously measured 2.26 rad/s; public driver unchanged')
