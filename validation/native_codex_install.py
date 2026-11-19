"""Use the production Codex installation SDK in unchanged source tests."""
import contextlib
import functools
import sys
from types import SimpleNamespace

from native_plugin import ROOT, invoke

sys.path.insert(0,str(ROOT / "src/experiments/sdk"))
from te_runtime.codex_install import bind


def install():
    from te_harness import codex as module
    originals={name:getattr(module,name) for name in ("codex_bin","codex_vendor","require_install","cli_version",
        "base_prompt_for","bundled_system_skills","validate_config","_kill_process_group","_rmtree_quietly")}
    bind(SimpleNamespace(request=invoke,frames=contextlib.nullcontext,callback_scope=contextlib.nullcontext,
        deliver_signals=lambda:None),module)
    for name,original in originals.items(): functools.update_wrapper(getattr(module,name),original)
    return {original:getattr(module,name) for name,original in originals.items()}
