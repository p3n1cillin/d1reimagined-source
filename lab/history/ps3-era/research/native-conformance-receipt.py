from pathlib import Path
from datetime import datetime, timezone
import json
import re
import sys

name, build = sys.argv[1:]
logs = Path(r'F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs')
text = (logs / (name + '-pre-stop-native.log')).read_text(encoding='utf-8')
last = re.findall(r'^·[^\n]*', text, re.M)[-1]
value = {'at': datetime.now(timezone.utc).isoformat(), 'build': build, 'fixture_completed': True,
         'last_native_line': last,
         'concurrent_queue_callbacks': len(re.findall(r'THREADING ERROR[^\n]*VkQueue', text)),
         'query_scope_callbacks': len(re.findall(r'vkCmdEndQuery\(\)', text)),
         'descriptor_reset_callbacks': len(re.findall(r'vkResetDescriptorPool\(\)', text)),
         'device_lost': len(re.findall(r'VK_ERROR_DEVICE_LOST', text)),
         'validation_ids': sorted(set(re.findall(r'\bVUID-[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+)+', text))),
         'scope': 'Bounded conformance comparison; visual/gameplay acceptance NOT RUN'}
(logs / (name + '-native-acceptance.json')).write_text(json.dumps(value, indent=2), encoding='utf-8')
print(json.dumps(value, indent=2))
