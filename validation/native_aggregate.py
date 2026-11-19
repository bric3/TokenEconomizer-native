"""Data and console binding for the native results aggregator."""
import collections
import inspect
import os
import sys

from native_plugin import invoke


def install(module):
    constants = invoke("experiment.aggregate.constants", {})
    assert module.FIELDS == constants["fields"]
    assert module.USD_PER_EFFECTIVE_TOKEN == constants["usd_per_effective_token"]
    assert module.AGENTS_WITHOUT_VENDOR_USD == tuple(constants["agents_without_vendor_usd"])
    module.FIELDS = constants["fields"]
    module.USD_PER_EFFECTIVE_TOKEN = constants["usd_per_effective_token"]
    module.AGENTS_WITHOUT_VENDOR_USD = tuple(constants["agents_without_vendor_usd"])

    def bind(name):
        signature = inspect.signature(getattr(module, name))
        def call(*args, **kwargs):
            arguments = signature.bind(*args, **kwargs)
            arguments.apply_defaults()
            values = {key: os.fspath(value) if isinstance(value, os.PathLike) else value
                      for key, value in arguments.arguments.items()}
            value = invoke("experiment.aggregate." + name, values)
            if name in ("usage_incomplete", "not_graded", "vendor_cost_missing", "cell_of", "_run_order"):
                return tuple(value)
            if name == "summarize":
                return collections.OrderedDict((tuple(cell["key"]), cell["value"]) for cell in value)
            if name == "collect":
                print(value["stdout"], end="")
                return value["rows"]
            return value
        return call

    for name in ("discover_arms", "cell_of", "normalize_repo", "_run_order", "usage_incomplete",
                 "not_graded", "vendor_usd", "vendor_cost_missing", "collect", "summarize"):
        setattr(module, name, bind(name))

    def main():
        value = invoke("experiment.aggregate.main", {"program": os.path.basename(sys.argv[0]), "argv": sys.argv[1:]})
        print(value["stdout"], end="")
        print(value["stderr"], end="", file=sys.stderr)
        if value["exit"] is not None:
            raise SystemExit(value["exit"])
    module.main = main
