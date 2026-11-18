"""JSON I/O for independent source/native harness-accounting comparisons."""
import contextlib
import io
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
from te_harness import swebench, codex, copilot

if "--native" in sys.argv:
    from native_experiment_usage import install
    install()


def data(value):
    if isinstance(value, dict):
        return {key: data(item) for key, item in value.items()}
    if isinstance(value, (set, frozenset)):
        return sorted((data(item) for item in value), key=repr)
    if isinstance(value, (list, tuple)):
        return [data(item) for item in value]
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return value


for line in sys.stdin:
    request = json.loads(line)
    output = io.StringIO()
    logs = []
    swebench._log = logs.append
    previous_cwd = os.getcwd()
    try:
        if "cwd" in request:
            os.chdir(request["cwd"])
        with contextlib.redirect_stdout(output):
            if request.get("domain") == "recovery":
                recovery = swebench._UsageRecovery()
                steps = []
                for operation in request["operations"]:
                    try:
                        value = getattr(recovery, operation["method"])(**operation.get("arguments", {}))
                        steps.append({"value": data(value), "state": data(list(recovery._by_msg.items()))})
                    except Exception as error:
                        steps.append({"raised": type(error).__name__, "message": str(error), "state": data(list(recovery._by_msg.items()))})
                value = steps
            else:
                module = {"claude": swebench, "codex": codex, "copilot": copilot}[request["domain"]]
                value = getattr(module, request["method"])(**request.get("arguments", {}))
        response = {"value": data(value), "logs": logs, "output": output.getvalue()}
    except Exception as error:
        response = {"raised": type(error).__name__, "message": str(error), "logs": logs, "output": output.getvalue()}
    finally:
        os.chdir(previous_cwd)
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
