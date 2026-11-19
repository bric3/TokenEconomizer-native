"""Register the production task bindings against original validation namespaces."""
import sys

from native_plugin import ROOT
from sdk_transport import ValidationBridge

sys.path.insert(0,str(ROOT/"src/experiments/sdk"))
from te_runtime.swebench_task import bind, bind_network, checked, data


def install_network(module):
    return bind_network(ValidationBridge(),module)


def install():
    from te_harness import swebench
    replacements=bind(ValidationBridge(),swebench,container_helpers=lambda:str(ROOT/"target/container-helpers"))
    web=sys.modules.get("te_harness.webbench")
    if web is not None:
        for name in ("answer_key_attempts","guard_accepts"):
            previous=getattr(web,name,None)
            if previous in replacements: setattr(web,name,replacements[previous])
        if hasattr(web,"network_tool_block"): replacements.update(install_network(web))
    return replacements
