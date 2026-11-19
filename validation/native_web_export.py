"""Bind original exporter entry points to native file and registry operations."""
import os
import sys

from native_plugin import ROOT, invoke
sys.path.insert(0, str(ROOT / "src/experiments/sdk"))
from te_runtime.transport import checked


def install(module):
    constants = checked(invoke("experiment.datasets.web_constants", {}))
    for name, value in constants.items():
        assert getattr(module, name) == value

    def build(projects_root):
        return checked(invoke("experiment.datasets.web_build", {
            "projects_root": os.fspath(projects_root), "process_cwd": os.getcwd()}))

    def main():
        value = checked(invoke("experiment.datasets.web_main", {
            "program": os.path.basename(sys.argv[0]), "argv": sys.argv[1:],
            "env": {key: os.environ[key] for key in constants.values() if key in os.environ},
            "process_cwd": os.getcwd()}))
        print(value["stdout"], end="")
        print(value["stderr"], end="", file=sys.stderr)
        if value["exit"] is not None:
            raise SystemExit(value["exit"])
        return value["returncode"]

    module.build, module.main = build, main
