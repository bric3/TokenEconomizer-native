"""Source API objects and I/O streams for answer-key guard comparisons."""
import contextlib
import io
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as sw
from te_harness.guards import net_guard
if "--native" in sys.argv:
    from native_experiment_task import install
    install()
    from native_experiment_net_guard import install
    install(net_guard)

for line in sys.stdin:
    request = json.loads(line)
    try:
        if request.get("method") == "answer_key_attempts":
            value = sw.answer_key_attempts(request["root"])
        elif request.get("method") == "main":
            saved = {key:os.environ.get(key) for key in ("TE_ALLOW_NETWORK_TOOLS","TE_NET_GUARD_LOG")}
            previous = sys.stdin
            try:
                for key in saved:
                    if key in request.get("env",{}): os.environ[key] = request["env"][key]
                    else: os.environ.pop(key,None)
                sys.stdin = io.StringIO(request.get("input",""))
                stderr = io.StringIO()
                with contextlib.redirect_stderr(stderr): value = net_guard.main()
                value = {"code":value,"stderr":stderr.getvalue()}
            finally:
                sys.stdin = previous
                for key,previous_value in saved.items():
                    if previous_value is None: os.environ.pop(key,None)
                    else: os.environ[key] = previous_value
        else: value = getattr(net_guard,request.get("method","hook_command"))(**request.get("args",{}))
        response = {"value":value}
    except BaseException as error:
        response = {"raised":type(error).__name__,"message":str(error)}
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
