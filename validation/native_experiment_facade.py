"""Object, adapter and runner I/O for the native experiment facade."""
import dataclasses
import functools
import inspect
import os

from native_plugin import (invoke, prefix_data, restore_prefix, tooling_data, restore_tooling, vocabulary_data)
from native_experiment_spiral import checked, stage_data, restore_stage, stage_invoke


def optimizer_callable(runner):
    arguments = {"present": runner is not None, "has_optimizer": hasattr(runner, "optimizer_call"), "callable": callable(runner)}
    method = checked(invoke("experiment.runner.optimizer_method", arguments))
    if isinstance(method, dict) and method.get("callback") == "representation":
        method = checked(invoke("experiment.runner.optimizer_method", dict(arguments, representation=repr(runner))))
    if method is None:
        return None
    if method == "__call__":
        return runner
    return lambda prompt, model="agent": getattr(runner, method)(prompt, model)


def facade_data(obj):
    return {"config": dataclasses.asdict(obj.config), "injection": prefix_data(obj.injection),
            "tooling": tooling_data(obj.tooling), "spiral": stage_data(obj.spiral)}


def restore(obj, data):
    from tokeneconomizer.config import EconomizerConfig, InjectionConfig, ToolingConfig, SpiralConfig
    from tokeneconomizer.microskills import PrefixStage
    from tokeneconomizer.toolprune import ToolingStage
    from te_bench.spiral_stage import SpiralStage
    if not hasattr(obj, "config"):
        obj.config = EconomizerConfig(InjectionConfig(**data["config"]["injection"]), ToolingConfig(**data["config"]["tooling"]), SpiralConfig(**data["config"]["spiral"]))
    obj.injection = getattr(obj, "injection", PrefixStage.__new__(PrefixStage))
    obj.tooling = getattr(obj, "tooling", ToolingStage.__new__(ToolingStage))
    obj.spiral = getattr(obj, "spiral", SpiralStage.__new__(SpiralStage))
    restore_prefix(obj.injection, data["injection"], obj._native_runners.get("injection"))
    obj.injection.adapter, obj.injection.agent = obj.adapter, data["tooling"]["agent"]
    obj.tooling.cfg, obj.tooling.adapter = obj.config.tooling, obj.adapter
    obj.tooling.judge_tool_runner = obj._native_runners.get("tooling")
    restore_tooling(obj.tooling, data["tooling"])
    obj.spiral.cfg, obj.spiral.adapter = obj.config.spiral, obj.adapter
    obj.spiral._native_runner = obj._native_runners.get("spiral")
    restore_stage(obj.spiral, data["spiral"])
    obj.spiral.spiral_judge = (lambda digest: stage_invoke(obj.spiral, "judge", {"digest": digest})) if data["spiral"]["spiral_judge"] else None
    obj.spiral.spiral_nudge_writer = (lambda digest: stage_invoke(obj.spiral, "nudge_writer", {"digest": digest})) if data["spiral"]["spiral_nudge_writer"] else None


def call(obj, name, arguments):
    from tokeneconomizer.microskills import distiller
    replies = []
    while True:
        result = checked(invoke("experiment.facade." + name, dict(arguments, replies=replies,
                         propose_every=distiller.PROPOSE_EVERY, home=os.path.expanduser("~"))))
        distiller.PROPOSE_EVERY = result["propose_every"]
        if "callback" not in result:
            for message in result["messages"]:
                print(message)
            if "self" in result:
                restore(obj, result["self"])
            return result
        try:
            operation = result["callback"]
            if operation == "default_adapter":
                from te_bench.adapters.swebench import SwebenchAdapter
                obj.adapter = SwebenchAdapter()
                value = None
            elif operation == "factory_available":
                value = bool(obj._native_factory)
            elif operation == "resolve":
                raw = obj._native_runner if result["source"] == "runner" else obj._native_factory()
                runner = optimizer_callable(raw)
                obj._native_runners[result["slot"]] = runner
                value = runner is not None
            elif operation == "adapter":
                method = result["method"]
                if method == "env_vocabulary":
                    vocabulary = obj.adapter.env_vocabulary() if hasattr(obj.adapter, method) else None
                    value = vocabulary_data(vocabulary) if vocabulary is not None else None
                elif method == "parse_catalog_with_source" and not hasattr(obj.adapter, method):
                    value = obj.adapter.parse_catalog(*result["args"]), "unknown"
                else:
                    value = getattr(obj.adapter, method)(*result["args"])
                if method == "protected_tools":
                    value = {"names": sorted(value), "patterns": list(getattr(value, "patterns", ()))}
                elif isinstance(value, (set, frozenset)):
                    value = sorted(value)
            elif operation == "runner":
                value = obj._native_runners[result["slot"]](result["prompt"], result["model"])
            elif operation == "estimate":
                value = obj._native_runners[result["slot"]].estimate_effective(result["prompt"], result["raw"])
            else:
                raise AssertionError(operation)
            replies.append({"value": value})
        except Exception as error:
            message = str(error) if operation == "adapter" and result["method"] == "problem_statements" else type(error).__name__ + ": " + str(error)
            replies.append({"error": message})


def install():
    from te_bench import runner as module
    from te_bench.facade import Economizer
    module.as_optimizer_callable = optimizer_callable
    original = Economizer.__init__
    @functools.wraps(original)
    def constructor(self, config, *, agent, repo, results_root, fresh, optimizer_model="agent", max_turns=80,
                    runner=None, make_runner=None, adapter=None):
        self.config, self.adapter = config, adapter
        self._native_runner, self._native_factory, self._native_runners = runner, make_runner, {}
        result = call(self, "new", {"config": dataclasses.asdict(config), "options": {"agent": agent, "repo": repo,
                      "results_root": os.fspath(results_root), "fresh": fresh, "optimizer_model": optimizer_model,
                      "max_turns": max_turns}, "adapter": adapter is not None, "runner": runner is not None})
        checked(result["value"])
    Economizer.__init__ = constructor

    @classmethod
    def from_args(cls, args, *, repo, results_root, fresh, runner=None, make_runner=None, adapter=None):
        obj = cls.__new__(cls)
        obj.adapter = adapter
        obj._native_runner, obj._native_factory, obj._native_runners = runner, make_runner, {}
        result = call(obj, "from_args", {"arguments": vars(args), "options": {"repo": repo, "results_root": os.fspath(results_root),
                      "fresh": fresh}, "adapter": adapter is not None, "runner": runner is not None})
        checked(result["value"])
        return obj
    Economizer.from_args = from_args

    def method(original, name):
        signature = inspect.signature(original)
        @functools.wraps(original)
        def native(self, *args, **kwargs):
            bound = signature.bind(self, *args, **kwargs)
            bound.apply_defaults()
            arguments = dict(bound.arguments)
            arguments["self"] = facade_data(self)
            arguments["runners"] = {key: value is not None for key, value in self._native_runners.items()}
            if "run_dir_path" in arguments:
                arguments["run_dir_path"] = os.fspath(arguments["run_dir_path"])
            result = call(self, name, arguments)
            if "srow" in bound.arguments:
                bound.arguments["srow"].clear()
                bound.arguments["srow"].update(result["srow"])
            value = checked(result["value"])
            if name == "run_kwargs":
                from te_bench.codex_spiral import CodexSpiralPolicy
                if "spiral_judge" in value:
                    value["spiral_judge"], value["spiral_nudge_writer"] = self.spiral.spiral_judge, self.spiral.spiral_nudge_writer
                if value.get("codex_spiral_policy") is not None:
                    value["codex_spiral_policy"] = CodexSpiralPolicy(**value["codex_spiral_policy"])
            return tuple(value) if name == "after_task" else value
        return native
    for name in ("injection_prefix", "build_tools", "run_kwargs", "after_task"):
        setattr(Economizer, name, method(getattr(Economizer, name), name))
