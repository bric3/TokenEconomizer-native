"""Use the production streamed-rollout controller SDK in unchanged source tests."""
import sys

from native_plugin import ROOT, invoke

sys.path.insert(0,str(ROOT / "src/experiments/sdk"))
from te_runtime.nudge_guard import bind, guard_data


def install():
    from te_bench import nudge_guard as module
    sdk=bind(invoke)
    module.RESUME_HEADER,module.DEFAULT_THRESHOLDS=sdk.RESUME_HEADER,sdk.DEFAULT_THRESHOLDS
    for name in ("__init__","_restore","_call","monitor","_sampled_judge","_nudge_body","continuation","_stop"):
        setattr(module.NudgeGuard,name,getattr(sdk.NudgeGuard,name))
