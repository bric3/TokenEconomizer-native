"""Original/native JSON file and setting-conversion bindings."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "reference/upstream"))
from tokeneconomizer.plugin.config import _read, load_config
if "--native" in sys.argv:
    from native_plugin import invoke

for line in sys.stdin:
    request = json.loads(line)
    try:
        if request["method"] == "read":
            value = invoke("live_config._read", dict(path=request["path"])) if "--native" in sys.argv else _read(request["path"])
        elif "--native" in sys.argv:
            value = invoke("live_config.load_config", dict(root=request["root"], repo_dir=None, env={}))
        else:
            value = vars(load_config(request["root"], env={}))
        output = dict(value=value)
    except Exception as error:
        output = dict(raised=type(error).__name__, message=str(error))
    print(json.dumps(output, sort_keys=True, ensure_ascii=True, separators=(",", ":")), flush=True)
