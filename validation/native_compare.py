"""Console and ordered mapping bindings for native paired-cell reports."""
import os
import sys
from native_plugin import ROOT, invoke
sys.path.insert(0, str(ROOT / "src/experiments/sdk"))
from te_runtime.transport import checked


def install(module):
    def call(name, **args):
        return checked(invoke("experiment.compare." + name, {**args, "process_cwd": os.getcwd()}))

    def load_cell(cell):
        return tuple(dict(rows) for rows in call("load_cell", cell=os.fspath(cell)))

    def describe(rows, cfgs, label):
        print(call("describe", rows=list(rows.items()), cfgs=list(cfgs.items()), label=label), end="")

    def main():
        value = invoke("experiment.compare.main", {"process_cwd": os.getcwd(),
            "program": os.path.basename(sys.argv[0]), "argv": sys.argv[1:]})
        print(value.get("stdout", ""), end="")
        value = checked(value)
        print(value["stderr"], end="", file=sys.stderr)
        if value["exit"] is not None: raise SystemExit(value["exit"])
        return value["returncode"]

    module.load_cell = load_cell
    module.cost = lambda r: call("cost", r=r)
    module.describe = describe
    module.main = main
