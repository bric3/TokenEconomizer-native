"""Load production scaffold bindings without reference namespaces or test transports."""
import atexit
import builtins
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
original_import=builtins.__import__
def guarded(name,*args,**kwargs):
    if name.split(".")[0] in ("tokeneconomizer","te_bench","te_harness","native_plugin","sdk_transport"):
        raise AssertionError("Scaffold SDK imported reference/test code: "+name)
    return original_import(name,*args,**kwargs)
builtins.__import__=guarded
sys.path.insert(0,str(ROOT/"src/experiments/sdk"))
from te_runtime.scaffolds import bind
from te_runtime.transport import Bridge

bridge=Bridge(ROOT/"target/debug/te-bridge")
atexit.register(bridge.close)
runtime=bind(bridge)
base=runtime.base
