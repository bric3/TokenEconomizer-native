"""Source SDK and subprocess stand-ins for Codex configuration comparisons."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as sw

if "--native" in sys.argv:
    from native_experiment_task import install
    install()

for line in sys.stdin:
    request = json.loads(line)
    calls,logs = [],[]
    saved = os.environ.get("TE_ALLOW_NETWORK_TOOLS")
    original_log = sw._log
    notes = request.get("notes",{"arm_setup":"fixture","mcp_config":"/fixture/mcp.json"})
    servers = notes.get("mcp_servers") if isinstance(notes,dict) else None
    mode = request.get("mode","config_toml")
    runner = object.__new__(sw.CodexSwebenchRunner)
    runner.__dict__.update(base_prompt=request.get("base_prompt"),_capabilities=request.get("capabilities",[]),
                           model="model-é😀",effort="high",routing={"base_url":"http://localhost/v1"},
                           _mcp_servers={"stale":{}},system_skills=None)
    def execute(*args,**kwargs):
        calls.append([args,kwargs])
        return subprocess.CompletedProcess(args,request.get("returncode",0),request.get("stdout","wrote\n"),request.get("stderr",""))
    def context(*args):
        calls.append(["context",args])
        return args
    class Scaffold:
        name = "fixture"
        def __bool__(self): return not request.get("falsey_scaffold")
        def setup(self,ctx):
            calls.append(["setup",ctx])
            if request.get("error"):
                raise getattr(__import__("builtins"),request["error"])(request.get("message","fixture failure"))
            return notes
    runner._exec = execute
    runner._scaffold_context = context
    runner.scaffold = None if request.get("no_scaffold") else Scaffold()
    try:
        if "network" in request: os.environ["TE_ALLOW_NETWORK_TOOLS"] = request["network"]
        else: os.environ.pop("TE_ALLOW_NETWORK_TOOLS",None)
        sw._log = logs.append
        if mode == "write_config":
            value = runner._write_config("fixture")
        elif mode in ("apply_scaffold","apply_codex_scaffold"):
            if mode == "apply_codex_scaffold":
                runner._write_config = lambda name:calls.append(["write_config",name,runner._mcp_servers])
                value = runner._apply_scaffold("fixture",{"instance_id":"iid"},"/fixture/run")
            else:
                value = sw.ClaudeSwebenchRunner._apply_scaffold(runner,"fixture",{"instance_id":"iid"},"/fixture/run")
            if isinstance(notes,dict) and not request.get("error") and not request.get("no_scaffold"):
                assert value[0] is notes,"scaffold notes lost identity"
            if mode == "apply_codex_scaffold" and servers:
                assert runner._mcp_servers is servers,"server specification lost identity"
        else:
            kwargs = request.get("kwargs",{})
            if mode == "config_toml": kwargs = {"routing":{"base_url":"http://localhost/v1"},"model":"model-é😀","effort":"high",**kwargs}
            value = getattr(sw.codex,mode)(**kwargs)
        response = {"value":value}
    except BaseException as error:
        response = {"raised":type(error).__name__,"message":str(error)}
    finally:
        response.update(calls=calls,logs=logs,notes=notes,mcp_servers=runner._mcp_servers,lean_tokens=getattr(runner,"_lean_instructions_tokens",None))
        sw._log = original_log
        if saved is None: os.environ.pop("TE_ALLOW_NETWORK_TOOLS",None)
        else: os.environ["TE_ALLOW_NETWORK_TOOLS"] = saved
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
