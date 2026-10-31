"""Fresh replay inputs: PR31 scheduled authority; exclude the failed member activation."""
from pathlib import Path
import shutil
p = Path(__file__).resolve().parent
staging = Path(r'F:\d1\d1preservation-main\cfw-staging')
run = staging/'2026-10-27-divide13'
run.mkdir(exist_ok=False)
for name in ['breach-prop3-only.txt','divide-squads-gen2.txt']:
    shutil.copyfile(staging/'2026-10-27-divide12'/name, run/name)
steps = (staging/'agr-steps-breach14-quiet.txt').read_text()
steps += f'164 %96|900 0x4786c0e0 {run.as_posix()}/divide-squads-gen2.txt\n'
(run/'steps.txt').write_text(steps)
source = (p/'replay-divide12.py').read_text().replace('2026-10-27-divide12','2026-10-27-divide13')
source = source.replace("'--tolerance', str(tolerance), *extra]", "'--tolerance', str(tolerance), '--turn-sign', '1', *extra]")
source = source.replace("    walk('breach', ROUTES/'breach.json'", "    walk('enter-wall', ROUTES/'enter_the_wall.json', 650)\n    walk('breach', ROUTES/'breach.json'")
source = source.replace("PRIVATE/'jw-route.json'", "PRIVATE/'jw-route-from4.json'")
source = source.replace("    publish(STAGING/'agr-s96-clients-auth.txt')", "    print('Scheduled Divide frame must be verified in AH log and native state; no manual slice-96 publish', flush=True)")
source = source.replace("DIVIDE READY: authority baseline published; squads remain unspawned for inspection", "DIVIDE READY: scheduled authority and squad frame awaits native readback")
(p/'replay-divide13.py').write_text(source)
print('Prepared fresh run 13, scheduled slice-96 frame; authored-member activation excluded')
