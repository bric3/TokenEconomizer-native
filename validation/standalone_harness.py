"""Bind signature-only test interfaces using the shipped SDK and owned engine."""
import atexit
import builtins
import inspect
from pathlib import Path
import signal
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
from te_runtime.optimizer import bind as optimizer_bindings
from te_runtime.codex_install import bind as install_bindings
from te_runtime.codex_spiral import bind as spiral_bindings
from te_runtime.nudge_guard import bind as guard_bindings
from te_runtime.scaffolds import bind as scaffold_bindings
from te_runtime.usage import bind as usage_bindings
from te_runtime.swebench_task import bind as task_bindings, bind_network
from te_runtime.runner_setup import bind as setup_bindings
from te_runtime.providers import bind as provider_bindings
import harness_interfaces as interfaces

bridge=Bridge(ROOT/"target/debug/te-bridge")
atexit.register(bridge.close)
optimizer=optimizer_bindings(bridge)
sw=optimizer.claude
codex,copilot=optimizer.codex,optimizer.copilot
for name,value in vars(interfaces).items():
    if not name.startswith("__") and name not in ("CodexInterface","CopilotInterface"):
        setattr(sw,name,value)
for module,interface in ((codex,interfaces.CodexInterface),(copilot,interfaces.CopilotInterface)):
    for name in vars(interface):
        if not name.startswith("__"): setattr(module,name,getattr(interface,name))
metadata=checked(bridge.request("experiment.harness.constants",{}))
for name,module in (("claude",sw),("copilot",copilot),("codex",codex)):
    for key,value in metadata[name].items():
        setattr(module,key,tuple(value) if key == "DEFAULT_SYSTEM_SKILLS" else frozenset(value))
sw.codex,sw.copilot=codex,copilot
sw.inspect=inspect
sw.scaffolds_pkg=scaffold_bindings(bridge).registry
sw.NudgeGuard=guard_bindings(bridge.request).NudgeGuard
sw.codex_spiral=spiral_bindings(bridge.request)
sw._LIVE_CONTAINERS=set()
codex.signal=signal
install_bindings(bridge,codex)
accounting=usage_bindings(bridge.request,lambda message:sw._log(message))
sw._merge_usage=accounting.claude._merge_usage
copilot.merge_usage=accounting.copilot.merge_usage
codex.merge_usage=accounting.codex.merge_usage
provider=provider_bindings(bridge)
for cls,agent in ((sw.ClaudeSwebenchRunner,"claude"),(sw.CopilotSwebenchRunner,"copilot"),(sw.CodexSwebenchRunner,"codex")):
    def _merge_segments(self,a,b): raise AssertionError("unbound fixture merge")
    cls._merge_segments=provider(sw,_merge_segments,"webbench","agent_merge_"+agent)
task_bindings(bridge,sw,container_helpers=lambda:str(ROOT/"target/container-helpers"))
setup_bindings(bridge,sw)
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
