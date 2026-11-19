"""Register production constructor bindings against original validation classes."""
import sys

from native_plugin import ROOT
from sdk_transport import ValidationBridge

sys.path.insert(0,str(ROOT/"src/experiments/sdk"))
from te_runtime.runner_setup import bind


def install(module=None):
    if module is None:
        from te_harness import swebench as module
    return bind(ValidationBridge(),module,web=module.__name__ == "te_harness.webbench")
