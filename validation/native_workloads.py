"""Original Python data and console bindings for native workload operations."""
import builtins
import inspect
import json
import os
import sys

from native_plugin import invoke


def checked(value):
    if "raised" not in value:
        return value["value"]
    if value["raised"] == "UnicodeDecodeError":
        raise UnicodeDecodeError("utf-8", bytes(value["bytes"]), value["start"], value["end"], value["message"])
    raise getattr(builtins, value["raised"])(value["message"])


def restore(template, value):
    if isinstance(template, tuple):
        return tuple(restore(t, v) for t, v in zip(template, value))
    if isinstance(template, frozenset):
        return frozenset(value)
    if isinstance(template, dict):
        return {key: restore(template[key], item) for key, item in value.items()}
    return value


def install(module):
    if vars(module).get("_native_workloads"):
        return
    if module.__name__ == "te_bench.workloads":
        constants = checked(invoke("experiment.workloads.constants", {}))
        for name, value in constants.items():
            original = getattr(module, name)
            assert json.loads(json.dumps(original, default=sorted)) == value
            setattr(module, name, restore(original, value))

        def bind(name):
            signature = inspect.signature(getattr(module, name))
            def call(*args, **kwargs):
                bound = signature.bind(*args, **kwargs)
                bound.apply_defaults()
                values = dict(bound.arguments)
                if name == "filter_tasks":
                    values["tasks"] = list(values["tasks"])
                value = checked(invoke("experiment.workloads." + name, values))
                if name == "instances":
                    return tuple(value)
                if name == "runnable_instances":
                    return {group: tuple(ids) for group, ids in value.items()} if isinstance(value, dict) else tuple(value)
                if name == "filter_tasks":
                    return [values["tasks"][index] for index in value]
                return value
            return call
        for name in ("instances", "runnable_instances", "task_counts", "total_tasks", "swebench_image_tag", "filter_tasks"):
            setattr(module, name, bind(name))
    else:
        module.build = lambda domain: checked(invoke("experiment.workloads.build", {"domain": domain}))
        module.path_for = lambda domain: checked(invoke("experiment.workloads.path_for", {"domain": domain, "out": os.fspath(module.OUT_DIR)}))
        def main():
            value = checked(invoke("experiment.workloads.main", {"program": os.path.basename(sys.argv[0]),
                "argv": sys.argv[1:], "out": os.fspath(module.OUT_DIR), "base": os.fspath(module.REPO)}))
            print(value["stdout"], end="")
            print(value["stderr"], end="", file=sys.stderr)
            if value["exit"] is not None:
                raise SystemExit(value["exit"])
            return value["returncode"]
        module.main = main
    module._native_workloads = True
