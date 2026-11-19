"""Original/native timestamp and trace-head transport, with a process-local time zone."""
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "reference/upstream"))
for arg in sys.argv[1:]:
    if arg.startswith("--tz="):
        os.environ["TZ"] = arg[5:]
        time.tzset()
from tokeneconomizer.trace import claude, droid
if "--native" in sys.argv:
    from native_plugin import invoke

for line in sys.stdin:
    request = json.loads(line)
    host = request.get("host", "claude")
    method = request.get("method", "_epoch")
    def call(args):
        if "--native" in sys.argv:
            return invoke("trace." + host + "." + method, args)
        return getattr(claude if host == "claude" else droid, method)(**args)
    if "batch" in request:
        value = [call({"ts": item}) for item in request["batch"]]
    else:
        value = call(request["args"])
    print(json.dumps(value, sort_keys=True, ensure_ascii=True, separators=(",", ":")), flush=True)
