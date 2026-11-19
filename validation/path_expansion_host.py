"""Original/native path and Claude-history I/O bindings."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "reference/upstream"))
from tokeneconomizer.plugin import hosts
claude = hosts.HOSTS["claude"]
if "--native" in sys.argv:
    from native_plugin import invoke, wrap
    import native_plugin
    native_plugin.NATIVE_SPAWN_FUNCTIONS["run"] = subprocess.run
    native_plugin.NATIVE_HOST_ANCESTORS = hosts._ancestor_args
    history = wrap(claude.history, "host_runtime.history")
else:
    history = claude.history

BASE = dict(os.environ)
for line in sys.stdin:
    request = json.loads(line)
    os.environ.clear()
    os.environ.update(BASE | request.get("env", {}))
    os.chdir(request.get("cwd", ROOT))
    try:
        if request.get("operation") == "history":
            extra = request.get("extra", [])
            if request.get("path_objects"):
                extra = [Path(path) for path in extra]
            value = history(claude(), request.get("history_env", {}), extra)
        elif "--native" in sys.argv:
            value = invoke("paths.expand_user", {"path": request["path"],
                "home": os.path.expanduser("~"), "env": dict(os.environ)})
        else:
            value = os.path.expanduser(request["path"])
        output = {"value": value}
    except Exception as error:
        output = {"raised": type(error).__name__, "message": str(error)}
    print(json.dumps(output, sort_keys=True, ensure_ascii=True, separators=(",", ":")), flush=True)
