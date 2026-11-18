"""Real subprocess, pipe and file bindings for owned Web-Bench agent checks."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"), str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_usage import install as usage_install
    usage_install()
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv: install(wb)

binary, destination = sys.argv[1:3]
root = Path(destination)
workspace = root/"workspace"
agent, mode = os.environ["TE_FIXTURE_AGENT"], os.environ["TE_FIXTURE_MODE"]
base = {"claude": wb.WebBenchRunner, "copilot": wb.CopilotWebBenchRunner, "codex": wb.CodexWebBenchRunner}[agent]
class Runner(base):
    def _write_config(self, *args): pass
    def agent_home(self, path): return str(root/"agent-home")
    def scaffold_copilot_home(self, path): return str(root/"copilot-home")
    def scaffold_bin_dir(self, path): return str(root/"bin")
    def _net_guard_log(self, home): return str(root/"guard.log")
class Guard:
    segment = 7
    def __bool__(self): return False
    def monitor(self, line): return True
runner = object.__new__(Runner)
for key, value in {"claude":binary,"copilot":binary,"agent_bin":binary,"model":"gpt-5.4","effort":"high","protected_tools":[],"api_key":"fixture-key","task_timeout":-1 if mode == "timeout" else 10,"_net_guard_armed":False,"answer_key_ns":False}.items(): setattr(runner,key,value)
actual_popen, processes = subprocess.Popen, []
def popen(*args, **kwargs):
    process = actual_popen(*args, **kwargs)
    if args[0][0] == binary: processes.append(process)
    return process
wb.subprocess.Popen = popen
wb._log = lambda value:None
swebench._log = lambda value:None
try:
    with (root/"transcript").open("w", encoding="utf-8") as handle:
        prompt = {"session_id":"fixture-session","prompt":"fixture prompt é😀\n","effort":"low"} if mode == "resume" else "fixture prompt é😀\n"
        try: value = runner._segment(str(workspace), prompt, handle, [], 8, Guard() if mode == "guard" else None)
        except RuntimeError as error:
            assert mode == "config_error" and str(error).startswith(agent+" refused"), str(error)
        else: assert mode != "config_error", "refused configuration was accepted"
    assert len(processes) == 1
    process = processes[0]
    code = process.wait(timeout=5)
    assert code == (2 if mode == "config_error" else 0 if mode in ("normal","resume") else -9), (code, mode)
    if mode == "config_error": pass
    elif agent == "claude":
        assert value["usage"]["input_tokens"] == 10 and value["usage"]["output_tokens"] == 4, value
        if mode != "normal": assert value["num_turns_recovered"] and value["usage_recovered_from"] == "session_log" and not value["usage_incomplete"], value
    else:
        assert value["input_tokens"] == (10 if mode in ("normal","resume") or agent == "copilot" else None), value
        assert value["num_turns"] == 1, value
        if agent == "copilot": assert '"prompt_tokens":60' in value["_debug_log"]
    transcript = (root/"transcript").read_text(encoding="utf-8")
    assert transcript == "" if mode == "config_error" else transcript.startswith('{"type":') and "\n   \n" not in transcript
finally:
    wb.subprocess.Popen = actual_popen
    for process in processes:
        if process.poll() is None: process.kill()
        process.wait(timeout=5)
        for stream in (process.stdin,process.stdout,process.stderr):
            if stream is not None and not stream.closed: stream.close()
if "--native" in sys.argv:
    import native_plugin
    assert native_plugin.CALLS["experiment.swebench.provider.webbench.segment_"+agent] == 1
    if agent != "claude":
        assert native_plugin.CALLS["experiment.usage."+agent+".row_usage"] == (0 if mode == "config_error" else 1)
        assert native_plugin.CALLS["experiment.swebench.provider."+agent+".invocation_"+("resume" if mode == "resume" else "build")] == 1
    if agent == "copilot": assert native_plugin.CALLS["experiment.swebench.provider.copilot.invocation_fatal"] == 1
    if agent == "claude" and mode != "normal":
        assert native_plugin.CALLS["experiment.swebench.provider.webbench.session_log"] == 1
        assert native_plugin.CALLS["experiment.usage.claude.recovery.synthesize"] == 1
print("Owned Web-Bench segment passed: "+agent+" "+mode,flush=True)
