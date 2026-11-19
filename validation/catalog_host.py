"""Source/native catalogue-capture transport with offline process stand-ins."""
import json
import contextlib
import os
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments"), str(ROOT / "src/experiments/sdk")]
from te_harness import capture_catalog as module
if "--native" in sys.argv:
    from native_plugin import invoke
    from te_runtime.catalog import bind
    bridge = types.SimpleNamespace(request=invoke, callback_scope=contextlib.nullcontext, deliver_signals=lambda:None)
    native = bind(bridge, module, install_recorder=lambda name:None)
else: native = module

for line in sys.stdin:
    request = json.loads(line)
    calls = []
    counters = {}
    errors = []
    output = Path(request.get("out", ROOT / "validation/results/catalog-output.json"))
    output.parent.mkdir(parents=True, exist_ok=True)
    if "prior" in request: output.write_text(request["prior"], encoding="utf-8")
    else: output.unlink(missing_ok=True)
    def record(name, *args):
        counters[name] = counters.get(name, 0) + 1
        calls.append([name, *args])
        if request.get("fail") == name and counters[name] == request.get("fail_at", 1):
            import builtins
            error = getattr(builtins, request.get("error", "RuntimeError"))("fixture failure")
            errors.append(error)
            raise error
    class Live(set):
        def add(self, name): record("live_add", name); return super().add(name)
        def discard(self, name): record("live_discard", name); return super().discard(name)
    def run(argv, **kwargs):
        shown = list(argv)
        if shown[-1].startswith("cat > /tmp/capture_recorder.py"): shown[-1] = "<recorder>"
        record("run", shown, kwargs)
        stdout, stderr, code = "", "", 0
        if argv[1] == "run":
            code = request.get("docker_code", 0)
            stderr = request.get("docker_stderr", "docker fixture failure")
        elif argv[-1].startswith("curl "):
            values = request.get("readiness", ["0"])
            index = counters.get("ready", 0)
            counters["ready"] = index + 1
            stdout = values[min(index, len(values)-1)]
        elif argv[-1].startswith("cat /tmp/tools.json"):
            values = request.get("captures", ['{"Write":"write files"}'])
            index = counters.get("capture", 0)
            counters["capture"] = index + 1
            stdout = values[min(index, len(values)-1)]
        elif argv[-1].startswith("tail -5"): stdout = "recorder fixture log"
        return types.SimpleNamespace(returncode=code, stdout=stdout, stderr=stderr)
    def agent(argv, **kwargs):
        record("agent", argv, kwargs)
        values = request.get("streams", ['{"type":"system","tools":["Write"]}'])
        index = counters.get("stream", 0)
        counters["stream"] = index + 1
        return types.SimpleNamespace(returncode=request.get("agent_code",0), stdout=values[min(index,len(values)-1)], stderr="")
    def popen(argv, **kwargs): record("popen", argv, kwargs); return object()
    def binary(): record("claude_bin"); return "/fixture/claude"
    def image(instance): record("image_for",instance); return "fixture-image:" + instance
    saved = {key:getattr(module,key) for key in ("os","subprocess","time","_run","_log","claude_bin","image_for","_LIVE_CONTAINERS")}
    module.os = types.SimpleNamespace(environ=request.get("env",{"ANTHROPIC_API_KEY":"fixture-key"}),path=os.path,makedirs=os.makedirs)
    module.subprocess = types.SimpleNamespace(run=agent, Popen=popen, DEVNULL=-3)
    module.time = types.SimpleNamespace(sleep=lambda seconds:record("sleep",seconds))
    module._run, module.claude_bin, module.image_for = run, binary, image
    module._log = lambda message:record("log",message)
    module._LIVE_CONTAINERS = Live()
    try:
        if request.get("operation") == "system_tools": value = native.system_tools(request.get("text"))
        else: value = native.capture(out_path=str(output), **request.get("options",{}))
        result = {"value":value}
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error),"same_error":bool(errors) and error is errors[-1]}
    finally:
        result.update(calls=calls, live=sorted(module._LIVE_CONTAINERS), file=output.read_text(encoding="utf-8") if output.is_file() else None)
        for key,value in saved.items(): setattr(module,key,value)
    print(json.dumps(result,sort_keys=True,ensure_ascii=True,separators=(",",":")),flush=True)
