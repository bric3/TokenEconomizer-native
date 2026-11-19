"""Original/native LiveConfig binding for Java-generated numeric cases."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "reference/upstream"))
from tokeneconomizer.plugin.config import LiveConfig
if "--native" in sys.argv:
    from native_plugin import wrap
    LiveConfig.set = wrap(LiveConfig.set, "live_config.set")

for line in sys.stdin:
    requests = json.loads(line)
    output = []
    for request in requests:
        config = LiveConfig()
        accepted = config.set(request["key"], request["value"], source="number probe")
        output.append(dict(accepted=accepted, value=config.as_dict(), warnings=config.warnings))
    print(json.dumps(output, sort_keys=True, ensure_ascii=True, separators=(",", ":")), flush=True)
