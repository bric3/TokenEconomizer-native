"""Run both production harness assemblies while rejecting reference imports."""
import atexit
import builtins
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
original_import=builtins.__import__
def guarded(name,*args,**kwargs):
    if name.split(".")[0] in ("tokeneconomizer","te_bench","te_harness","native_plugin","sdk_transport"):
        raise AssertionError("Harness SDK imported reference/test code: "+name)
    return original_import(name,*args,**kwargs)
builtins.__import__=guarded
sys.path.insert(0,str(ROOT/"src/experiments/sdk"))
from te_runtime.transport import Bridge
from te_runtime.swebench import bind
from te_runtime.webbench import bind as webbench_bindings

bridge=Bridge(ROOT/"target/debug/te-bridge")
atexit.register(bridge.close)
sw=bind(bridge,container_helpers=lambda:str(ROOT/"target/container-helpers"))
codex,copilot=sw.codex,sw.copilot
# Preserve source API metadata in differential callback descriptions.
sw.__name__="te_harness.swebench"
wb=webbench_bindings(bridge,shared=sw)
wb.__name__="te_harness.webbench"
