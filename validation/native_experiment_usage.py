"""Use production accounting bindings in unchanged source tests."""
import functools
import sys

from native_plugin import ROOT, invoke

sys.path.insert(0,str(ROOT / "src/experiments/sdk"))
from te_runtime.usage import bind


def install():
    from te_harness import swebench, codex, copilot
    sdk=bind(invoke,lambda message:swebench._log(message))
    replacements={}
    for module,bindings in ((swebench,sdk.claude),(codex,sdk.codex),(copilot,sdk.copilot)):
        for name,native in vars(bindings).items():
            if name == "_UsageRecovery":
                for method in ("__init__","_call","observe","synthesize"):
                    function=getattr(native,method)
                    if hasattr(module._UsageRecovery,method): functools.update_wrapper(function,getattr(module._UsageRecovery,method))
                    setattr(module._UsageRecovery,method,function)
            else:
                original=getattr(module,name)
                replacements[original]=functools.update_wrapper(native,original)
                setattr(module,name,native)
    return replacements
