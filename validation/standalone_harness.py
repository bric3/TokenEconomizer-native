"""Run the production SWE-bench assembly while rejecting reference imports."""
import atexit
import builtins
from pathlib import Path
import sys
from types import SimpleNamespace

ROOT=Path(__file__).resolve().parents[1]
original_import=builtins.__import__
def guarded(name,*args,**kwargs):
    if name.split(".")[0] in ("tokeneconomizer","te_bench","te_harness","native_plugin","sdk_transport"):
        raise AssertionError("Harness SDK imported reference/test code: "+name)
    return original_import(name,*args,**kwargs)
builtins.__import__=guarded
sys.path.insert(0,str(ROOT/"src/experiments/sdk"))
from te_runtime.transport import Bridge, checked
from te_runtime.swebench import bind
from te_runtime.swebench_task import bind_network
from te_runtime.runner_setup import bind as setup_bindings
from te_runtime.providers import bind as provider_bindings
import harness_interfaces as interfaces

bridge=Bridge(ROOT/"target/debug/te-bridge")
atexit.register(bridge.close)
sw=bind(bridge,container_helpers=lambda:str(ROOT/"target/container-helpers"))
codex,copilot=sw.codex,sw.copilot
# Preserve source API metadata in differential callback descriptions.
sw.__name__="te_harness.swebench"
metadata=checked(bridge.request("experiment.harness.constants",{}))
provider=provider_bindings(bridge)
wb=SimpleNamespace(network_tool_block=interfaces.network_tool_block)
bind_network(bridge,wb)
for name in ("os","shutil","subprocess","scaffolds_pkg","pricing","codex","copilot","resolve_anthropic_key","claude_bin","cli_version","_log","EFFORT_LEVELS","PROTECTED_TOOLS","_ANTHROPIC_DIRECT"):
    setattr(wb,name,getattr(sw,name))
for key,value in metadata["web"].items():
    setattr(wb,key,frozenset(value) if key == "WEBBENCH_SCAFFOLDS" else value)
for name in ("WebBenchRunner","CopilotWebBenchRunner","CodexWebBenchRunner","webbench_repo","node_bin","webbench_task_timeout","webbench_undici_abort"):
    setattr(wb,name,getattr(interfaces,name))
for name in ("webbench_repo","node_bin","webbench_task_timeout","webbench_undici_abort"):
    setattr(wb,name,provider(wb,getattr(wb,name),"webbench",name))
cls=wb.CodexWebBenchRunner
cls._probe_namespace_guard=provider(wb,cls._probe_namespace_guard,"webbench","_probe_namespace_guard")
setup_bindings(bridge,wb,web=True)
