"""Host objects and shared-task stand-ins for agent wrapper comparisons."""
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as sw

if "--native" in sys.argv:
    from native_experiment_task import install
    install()

def data(value):
    if isinstance(value,os.PathLike): return os.fspath(value)
    if isinstance(value,dict): return {key:data(item) for key,item in value.items()}
    if isinstance(value,(list,tuple)): return [data(item) for item in value]
    return value

for line in sys.stdin:
    request = json.loads(line)
    calls,logs = [],[]
    root = Path(request["root"])
    path = root/"transcript.jsonl"
    out_dir = Path(os.path.relpath(root)) if request.get("relative") else root
    original_base,original_log = sw.ClaudeSwebenchRunner.run_task,sw._log
    agent = request.get("agent","codex")
    runner = object.__new__(sw.CopilotSwebenchRunner if agent == "copilot" else sw.CodexSwebenchRunner)
    runner.__dict__.update(_capabilities=("stale",),_mcp_servers={"stale":{}},_guard_python="stale",scaffold_name=request.get("scaffold"))
    if "lean_tokens" in request: runner._lean_instructions_tokens = request["lean_tokens"]
    row = request.get("row",{"keep":{"nested":"identity"}})
    task = {"instance_id":"fixture"}
    excluded = ["fixture tool"]
    def base(self,actual_task,**kwargs):
        assert self is runner and actual_task is task and kwargs["exclude_tools"] is excluded,"wrapper copied shared-task inputs"
        calls.append({"task":actual_task,"kwargs":data(kwargs),"capabilities":data(self._capabilities),"mcp_servers":self._mcp_servers,"guard_python":self._guard_python})
        if request.get("error"): raise getattr(__import__("builtins"),request["error"])("fixture failure")
        if "capabilities_after" in request: self._capabilities = tuple(request["capabilities_after"])
        return row,out_dir
    try:
        sw.ClaudeSwebenchRunner.run_task,sw._log = base,logs.append
        mode = request.get("mode","run")
        if mode == "run":
            value = runner.run_task(task,arm="arm",injected="instructions",exclude_tools=excluded,run_dir_path=out_dir,**request.get("kwargs",{}))
            assert value[0] is row and value[1] is out_dir,"wrapper copied shared-task outputs"
            value = data(value)
        elif mode == "_unwrap_shell": value = sw.codex._unwrap_shell(request.get("command"))
        elif mode == "rtk_compliance": value = sw.codex.rtk_compliance(None if request.get("null_path") else path)
        else: value = sw.codex.mcp_tool_calls(None if request.get("null_path") else path,server=request.get("server"))
        response = {"value":value}
    except BaseException as error:
        response = {"raised":type(error).__name__,"message":str(error)}
    finally:
        debug = root/"copilot_debug.log"
        response.update(row=row,calls=calls,logs=logs,capabilities=data(runner._capabilities),mcp_servers=runner._mcp_servers,guard_python=runner._guard_python,debug_bytes=list(debug.read_bytes()) if debug.is_file() else None)
        sw.ClaudeSwebenchRunner.run_task,sw._log = original_base,original_log
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
