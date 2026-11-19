"""Actual process, pipe and filesystem bindings for offline Docker agent segments."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import types

ROOT=Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_harness import sw as swe
else:
    sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_harness import swebench as swe
if "--native" in sys.argv and "--sdk" not in sys.argv:
    from native_experiment_usage import install as usage_install
    usage_install()
    from native_experiment_providers import install
    install()
root=Path(sys.argv[1]);agent,mode=os.environ["TE_FIXTURE_AGENT"],os.environ["TE_FIXTURE_MODE"]
base={"claude":swe.ClaudeSwebenchRunner,"copilot":swe.CopilotSwebenchRunner,"codex":swe.CodexSwebenchRunner}[agent]
class Runner(base):
    NET_GUARD_LOG="/tmp/fixture-guard.log"
    scaffold_profile=types.SimpleNamespace(home="/tmp/agent-home",mcp_args=lambda cfg:[])
    def _env_activation_args(self,name): return ["-e","BASH_ENV=/testbed/project-env"]
class Guard:
    segment=7
    def __bool__(self): return False
    def monitor(self,line): return True
runner=object.__new__(Runner)
for key,value in {"model":"gpt-5.4","effort":"high","protected_tools":[],"api_key":"fixture-private-key","task_timeout":-1 if mode == "timeout" else 30,"_guard_python":None}.items():setattr(runner,key,value)
actual_popen,processes=subprocess.Popen,[]
def popen(*args,**kwargs):
    process=actual_popen(*args,**kwargs);processes.append(process);return process
swe.subprocess=types.SimpleNamespace(**(vars(subprocess)|{"Popen":popen}))
swe.time=types.SimpleNamespace(time=time.time,sleep=lambda seconds:None)
swe._log=lambda value:None
try:
    with root.joinpath("transcript").open("w",encoding="utf-8") as transcript:
        prompt={"session_id":"fixture-session","prompt":"fixture prompt é😀\n","effort":"low"} if mode == "resume" else "fixture prompt é😀\n"
        try:value=runner._segment("fixture-container",prompt,transcript,[],8,Guard() if mode == "guard" else None)
        except RuntimeError as error:assert mode == "config_error" and str(error).startswith(agent+" refused"),str(error)
        else:assert mode != "config_error","refused configuration was accepted"
    assert len(processes) == 1
    process=processes[0];process.wait(timeout=5)
    if mode == "config_error":assert process.returncode == 2
    elif agent == "claude":
        assert value["usage"]["input_tokens"] == 10 and value["usage"]["output_tokens"] == 4,value
        if mode != "normal":assert value["num_turns_recovered"] and value["usage_recovered_from"] == "session_log" and not value["usage_incomplete"],value
    else:
        assert value["input_tokens"] == (10 if mode in ("normal","resume") or agent == "copilot" else None),value
        assert value["num_turns"] == 1,value
        if agent == "copilot":assert '"prompt_tokens":60' in value["_debug_log"]
    contents=root.joinpath("transcript").read_text(encoding="utf-8")
    if mode == "config_error":assert contents == ""
    else:
        assert contents.startswith('{"type":') and "\n   \n" not in contents
        assert root.joinpath("workspace/src/a.js").read_text(encoding="utf-8") == "fixture answer"
    if mode in ("guard","timeout"):
        matcher="-x" if agent == "codex" else "-f"
        assert root.joinpath("signals").read_text(encoding="utf-8") == f"-TERM {matcher} {agent}\n-KILL {matcher} {agent}\n"
    elif root.joinpath("signals").exists():raise AssertionError("unexpected termination")
finally:
    for process in processes:
        if process.poll() is None:process.kill()
        process.wait(timeout=5)
        for stream in (process.stdin,process.stdout,process.stderr):
            if stream is not None and not stream.closed:stream.close()
if "--native" in sys.argv and "--sdk" not in sys.argv:
    import native_plugin
    assert native_plugin.CALLS["experiment.swebench.provider.swebench.segment_"+agent] == 1
    if agent != "claude":
        assert native_plugin.CALLS["experiment.usage."+agent+".row_usage"] == (0 if mode == "config_error" else 1)
        assert native_plugin.CALLS["experiment.swebench.provider."+agent+".invocation_"+("resume" if mode == "resume" else "build")] == 1
    if agent == "claude" and mode != "normal":
        assert native_plugin.CALLS["experiment.swebench.provider.swebench.session_log"] == 1
        assert native_plugin.CALLS["experiment.usage.claude.recovery.synthesize"] == 1
print("Owned SWE-bench segment passed: "+agent+" "+mode,flush=True)
