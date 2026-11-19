"""Original stage bindings and the production Codex controller SDK."""
import dataclasses
import os
import sys

from native_plugin import ROOT, invoke

sys.path.insert(0,str(ROOT / "src/experiments/sdk"))
from te_runtime.codex_spiral import bind, state_data
from te_runtime.transport import checked


def stage_data(obj):
    return {"cfg": dataclasses.asdict(obj.cfg), "max_turns": obj.max_turns, "optimizer_model": obj._native_optimizer_model,
            "agent": obj.agent, "envelope": vars(obj.envelope), "spiral_judge": obj.spiral_judge is not None,
            "spiral_nudge_writer": obj.spiral_nudge_writer is not None,
            "codex_tracker": vars(obj.codex_tracker) if obj.codex_tracker is not None else None}


def restore_stage(obj, data):
    from te_bench.codex_spiral import CodexEnvelopeTracker
    from tokeneconomizer.loopguard import EnvelopeTracker
    obj.max_turns, obj.agent = data["max_turns"], data["agent"]
    obj._native_optimizer_model = data["optimizer_model"]
    if not hasattr(obj, "envelope"):
        obj.envelope = EnvelopeTracker.__new__(EnvelopeTracker)
    vars(obj.envelope).update(data["envelope"])
    if data["codex_tracker"] is not None:
        if getattr(obj, "codex_tracker", None) is None:
            obj.codex_tracker = CodexEnvelopeTracker.__new__(CodexEnvelopeTracker)
        vars(obj.codex_tracker).update(data["codex_tracker"])
    else:
        obj.codex_tracker = None


def stage_call(obj, name, data):
    replies = []
    while True:
        result = checked(invoke("spiral_stage." + name, dict(data, replies=replies)))
        if "callback" not in result:
            return result
        try:
            if result["callback"] == "snapshot":
                value = obj.adapter.spiral_snapshot(result["path"])
            elif result["callback"] == "runner":
                value = obj._native_runner(result["prompt"], result["model"])
            elif result["callback"] == "estimate":
                value = obj._native_runner.estimate_effective(result["prompt"], result["raw"])
            replies.append({"value": value})
        except Exception as error:
            replies.append({"error": str(error)})


def stage_constructor(self, cfg, max_turns, runner=None, optimizer_model="agent", adapter=None, agent=None):
    self.cfg, self.adapter, self._native_runner = cfg, adapter, runner
    result = stage_call(self, "new", {"cfg": dataclasses.asdict(cfg), "max_turns": max_turns, "optimizer_model": optimizer_model, "agent": agent})
    data = result["self"]
    restore_stage(self, data)
    self.spiral_judge = (lambda digest: stage_invoke(self, "judge", {"digest": digest})) if data["spiral_judge"] else None
    self.spiral_nudge_writer = (lambda digest: stage_invoke(self, "nudge_writer", {"digest": digest})) if data["spiral_nudge_writer"] else None


def stage_invoke(self, name, data):
    result = stage_call(self, name, {"self": stage_data(self), "runner": self._native_runner is not None,
                                   "adapter": self.adapter is not None, **data})
    restore_stage(self, result["self"])
    return checked(result["value"])


def stage_kwargs(self, task):
    from te_bench.codex_spiral import CodexSpiralPolicy
    value = stage_invoke(self, "run_kwargs", {})
    if "spiral_judge" in value:
        value["spiral_judge"] = self.spiral_judge
        value["spiral_nudge_writer"] = self.spiral_nudge_writer
    if value.get("codex_spiral_policy") is not None:
        value["codex_spiral_policy"] = CodexSpiralPolicy(**value["codex_spiral_policy"])
    return value


def stage_after(self, task, row, run_dir_path, srow):
    result = stage_call(self, "after_task", {"self": stage_data(self), "row": row, "srow": srow,
                                           "run_dir_path": os.fspath(run_dir_path), "adapter": self.adapter is not None})
    restore_stage(self, result["self"])
    srow.clear()
    srow.update(result["srow"])
    return checked(result["value"])


def install():
    from te_bench import codex_spiral as cs
    from te_bench.spiral_stage import SpiralStage
    vars(cs).update(vars(bind(invoke)))
    SpiralStage.__init__, SpiralStage.run_kwargs, SpiralStage.after_task = stage_constructor, stage_kwargs, stage_after
