"""JSON I/O and scripted callbacks for source/native controller comparisons."""
import contextlib
import io
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    import atexit
    import builtins
    original_import=builtins.__import__
    def guarded(name,*args,**kwargs):
        if name.split(".")[0] in ("tokeneconomizer","te_bench","te_harness","native_plugin"):
            raise AssertionError("Nudge SDK imported reference/test code: "+name)
        return original_import(name,*args,**kwargs)
    builtins.__import__=guarded
    sys.path.insert(0,str(ROOT / "src/experiments/sdk"))
    from te_runtime.nudge_guard import bind, guard_data
    from te_runtime.transport import Bridge
    bridge=Bridge(ROOT / "target/debug/te-bridge")
    atexit.register(bridge.close)
    sdk=bind(bridge.request)
    NudgeGuard,SpiralState,DEFAULT_THRESHOLDS=sdk.NudgeGuard,sdk.SpiralState,sdk.DEFAULT_THRESHOLDS
else:
    sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
    from te_bench.nudge_guard import NudgeGuard, SpiralState, DEFAULT_THRESHOLDS
    from native_experiment_nudge import guard_data
    if "--native" in sys.argv:
        from native_experiment_nudge import install
        install()


def data(value):
    if isinstance(value, dict):
        return {key: data(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [data(item) for item in value]
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return value


for line in sys.stdin:
    request = json.loads(line)
    messages = io.StringIO()
    calls = []
    indices = {"judge": 0, "writer": 0}
    def callback(name):
        def invoke(digest):
            calls.append({"callback": name, "digest": digest})
            index = indices[name]
            indices[name] += 1
            replies = request[name]
            reply = replies[index] if index < len(replies) else None
            if isinstance(reply, dict) and "error" in reply:
                raise RuntimeError(reply["error"])
            return reply
        return invoke
    try:
        with contextlib.redirect_stdout(messages):
            if request.get("kind") == "state":
                state=SpiralState()
                before=state.snapshot()
                state.update("Edit",{"file_path":"src/a.py"},"edited")
                state.update("Bash",{"command":"pytest"},"FAILED test_a.py::test_a",failed=True)
                state.update("Bash",{"command":"pytest"},"FAILED test_a.py::test_a",failed=True)
                state.edit_counts["a.py"]=4
                value=dict(before=before,after=state.snapshot(),digest=state.recent_digest(n=request.get("n",24),envelope=4),
                    churn=state.edit_churn,files=state.distinct_edit_files,stagnation=state.stagnation)
            else:
                arguments = dict(request.get("arguments", {}))
                if request.get("thresholds") is not None:
                    arguments["thr"] = dict(DEFAULT_THRESHOLDS, **request["thresholds"])
                guard = NudgeGuard(**arguments, judge=callback("judge") if "judge" in request else None,
                                   nudge_writer=callback("writer") if "writer" in request else None)
                steps = []
                for operation in request.get("operations", []):
                    for key, value in operation.get("mutate", {}).items():
                        setattr(guard, key, value)
                    for key, value in operation.get("state", {}).items():
                        setattr(guard.st, key, value)
                    try:
                        result = getattr(guard, operation["method"])(**operation.get("arguments", {}))
                        steps.append({"value": result, "state": data(guard_data(guard))})
                    except Exception as error:
                        steps.append({"raised": type(error).__name__, "message": str(error), "state": data(guard_data(guard))})
                value = {"steps": steps, "state": data(guard_data(guard)), "calls": calls}
        response = {"value": value, "messages": messages.getvalue()}
    except Exception as error:
        response = {"raised": type(error).__name__, "message": str(error), "messages": messages.getvalue()}
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
