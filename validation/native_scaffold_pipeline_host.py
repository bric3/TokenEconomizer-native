"""SDK and process bindings for native scaffold merge pipelines."""
import json
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench,webbench
from te_bench.baselines import base
from native_experiment_task import install as install_task
from native_experiment_providers import install
from native_plugin import CALLS
install_task()
install()
install(runner_module=webbench)

for line in sys.stdin:
    request=json.loads(line);before=dict(CALLS);logs=[]
    saved_logs=swebench._log,webbench._log
    swebench._log=webbench._log=logs.append
    try:
        directory=Path(request["directory"])
        agent=request.get("agent","claude")
        if request["domain"] == "swe":
            cls={"claude":swebench.ClaudeSwebenchRunner,"copilot":swebench.CopilotSwebenchRunner,"codex":swebench.CodexSwebenchRunner}[agent]
            runner=object.__new__(cls)
            runner.scaffold_profile=base.AgentProfile(kind=agent,home=request["home"],instructions_file="AGENTS.md",nav_tools=(),mcp_args=lambda cfg:[])
            if "helper_error" in request:
                def fail(*args,**kwargs): raise getattr(__import__("builtins"),request["helper_error"])("helper fixture failure")
                runner._exec=fail
            context=runner._scaffold_context(request["container"],{},str(directory))
            if request.get("rebind_profile"): runner.scaffold_profile=None
        else:
            cls={"claude":webbench.WebBenchRunner,"copilot":webbench.CopilotWebBenchRunner,"codex":webbench.CodexWebBenchRunner}[agent]
            runner=object.__new__(cls)
            context=runner._scaffold_context(str(directory/"workspace"),request["home"],{},str(directory))
        merged=context.merge_json_file(request["path"],request["patch"])
        guarded=runner._install_net_guard(request["container"]) if request.get("guard") else None
        output={"merged":merged,"guarded":guarded,"temporary_files":[path.name for path in directory.glob("_scaffold_merge_*")]}
    except BaseException as error: output={"raised":type(error).__name__,"message":str(error)}
    finally: swebench._log,webbench._log=saved_logs
    output["logs"]=logs
    output["calls"]={name:count-before.get(name,0) for name,count in CALLS.items() if count>before.get(name,0)}
    print(json.dumps(output,sort_keys=True,separators=(",",":")),flush=True)
