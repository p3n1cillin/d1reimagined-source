from datetime import datetime, timezone
from pathlib import Path
import sys
import time

sys.path.insert(0, r'F:\d1\d1xenia\tools\content_capture')
from capture import lock

path = Path(r'F:\d1\<pc>-import\d1xenia\local\content-capture\status.lock')
with lock(path):
    print('Status lock held for six-second contention test at', datetime.now(timezone.utc).isoformat(), flush=True)
    time.sleep(6)
print('Status lock released at', datetime.now(timezone.utc).isoformat(), flush=True)
