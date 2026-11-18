"""JSON bindings for the native answer-key guard."""
import functools
import inspect
import os
import subprocess

from native_plugin import ROOT, invoke
from native_experiment_spiral import checked

def install(module=None):
    if module is None:
        from te_harness.guards import net_guard as module
    if hasattr(module,"_native_replacements"): return module._native_replacements
    replacements = {}
    def function(original,name):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(*args,**kwargs):
            bound = signature.bind(*args,**kwargs)
            bound.apply_defaults()
            result = checked(invoke("experiment.net_guard."+name,dict(bound.arguments)))
            return tuple(result) if name == "decide" else result
        replacements[original] = native
        return native
    for name in ("hosts_block","decide","hook_command","_as_command","_find_command"):
        setattr(module,name,function(getattr(module,name),name))
    original_main = module.main
    @functools.wraps(original_main)
    def main():
        if invoke("experiment.net_guard.lifted",{"value":module.os.environ.get("TE_ALLOW_NETWORK_TOOLS","")}): return 0
        try: payload = module.sys.stdin.read()
        except Exception: payload = ""
        payload = payload.encode("utf-8",errors="surrogateescape") if isinstance(payload,str) else payload
        result = subprocess.run([str(ROOT/"target/debug"/("te-net-guard.exe" if os.name == "nt" else "te-net-guard"))],input=payload,capture_output=True,env=dict(module.os.environ))
        module.sys.stderr.write(result.stderr.decode("utf-8",errors="surrogateescape"))
        return result.returncode
    module.main = main
    replacements[original_main] = main
    module._native_replacements = replacements
    return replacements
