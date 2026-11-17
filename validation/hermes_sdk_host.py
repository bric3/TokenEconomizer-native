"""SDK fixture I/O: load a binding, provide a thread-bound profile and call its hooks."""
import concurrent.futures
import contextvars
import importlib.util
import json
import os
import sqlite3
import sys
import threading
import types

profile = contextvars.ContextVar("hermes_profile")
constants = types.ModuleType("hermes_constants")
constants.get_hermes_home = profile.get
sys.modules["hermes_constants"] = constants
spec = importlib.util.spec_from_file_location("hermes_sdk_fixture", sys.argv[1])
binding = importlib.util.module_from_spec(spec)
spec.loader.exec_module(binding)
bridge = binding.Bridge()
output = threading.Lock()


def call(request):
    token = profile.set(request.get("home"))
    try:
        if request["method"] == "sql":
            with sqlite3.connect(request["db"]) as connection:
                connection.executescript(request["sql"])
            value = None
        elif request["method"] == "chdir":
            os.chdir(request["path"])
            value = None
        elif request["method"] == "kill_native":
            process = bridge._connection[0]
            process.kill()
            process.wait()
            value = None
        elif request["method"] == "snapshot":
            if hasattr(bridge, "sessions"):
                value = {key: {k: v for k, v in row.items() if k != "lock"}
                         for key, row in bridge.sessions.items()}
            else:
                value = bridge._request("snapshots", {})
        else:
            value = getattr(bridge, request["method"])(**request.get("args", {}))
        packet = {"id": request["id"], "value": value}
    except Exception as error:
        packet = {"id": request["id"], "error": str(error)}
    finally:
        profile.reset(token)
    with output:
        print(json.dumps(packet, separators=(",", ":")), flush=True)


try:
    with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
        for line in sys.stdin:
            pool.submit(call, json.loads(line))
finally:
    if hasattr(bridge, "close"):
        bridge.close()
