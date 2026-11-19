"""Original/native LiveConfig binding for Java-generated numeric cases."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "reference/upstream"))
from tokeneconomizer.plugin.config import LiveConfig, load_config, write_config
if "--native" in sys.argv:
    from native_plugin import wrap
    LiveConfig.set = wrap(LiveConfig.set, "live_config.set")
    load_config = wrap(load_config, "live_config.load_config")
    write_config = wrap(write_config, "live_config.write_config")

for line in sys.stdin:
    requests = json.loads(line)
    output = []
    for request in requests:
        try:
            if request.get("action") == "write":
                output.append(dict(value=write_config(request["path"], request["updates"])))
                continue
            if request.get("action") == "load":
                config = load_config(request["root"], repo_dir=request.get("repo"), env=request.get("env", {}))
                accepted = None
            else:
                config = LiveConfig()
                accepted = config.set(request["key"], request["value"], source="number probe")
            output.append(dict(accepted=accepted, value=config.as_dict(), warnings=config.warnings))
        except Exception as error:
            output.append(dict(raised=type(error).__name__, message=str(error)))
    print(json.dumps(output, sort_keys=True, ensure_ascii=True, separators=(",", ":")), flush=True)
