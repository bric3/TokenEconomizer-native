"""Argument, tuple and console bindings for the native sweep planner."""
import inspect
import json
import os
import sys

from native_plugin import ROOT, invoke
from native_workloads import restore

sys.path.insert(0, str(ROOT / "src/experiments/sdk"))
from te_runtime.transport import checked


def install(module):
    constants = checked(invoke("experiment.sweep.constants", {}))
    for name, value in constants.items():
        original = getattr(module, name)
        assert json.loads(json.dumps(original)) == value
        setattr(module, name, restore(original, value))

    def bind(name):
        signature = inspect.signature(getattr(module, name))
        def call(*args, **kwargs):
            bound = signature.bind(*args, **kwargs)
            bound.apply_defaults()
            values = {key: os.fspath(value) if isinstance(value, os.PathLike) else value
                      for key, value in bound.arguments.items()}
            value = checked(invoke("experiment.sweep." + name, {**values, "root": os.fspath(module._EXPERIMENTS), "process_cwd": os.getcwd()}))
            return [tuple(cell) for cell in value] if name == "cells" else value
        return call

    for name in ("group_sizes", "cells", "estimate", "_fmt_estimate"):
        setattr(module, name, bind(name))

    def main():
        value = checked(invoke("experiment.sweep.main", {"root": os.fspath(module._EXPERIMENTS), "process_cwd": os.getcwd(),
            "program": os.path.basename(sys.argv[0]), "argv": sys.argv[1:]}))
        print(value["stdout"], end="")
        print(value["stderr"], end="", file=sys.stderr)
        if value["exit"] is not None:
            raise SystemExit(value["exit"])
        return value["returncode"]
    module.main = main
