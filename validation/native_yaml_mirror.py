"""Original API bindings for the native YAML mirror and its SDK value codec."""
import os
import sys

from native_plugin import ROOT, invoke
from sdk_transport import ValidationBridge
sys.path.insert(0, str(ROOT / "src/experiments/sdk"))
from te_runtime.datasets import Exporter
from te_runtime.transport import checked


def install(module):
    exporter = Exporter(ValidationBridge())
    assert module.FIELDS == tuple(exporter.constants["yaml_fields"])
    assert module.PROJECTS_ROOT_ENV == exporter.constants["projects_env"]

    def projects_root(explicit=None):
        encoded = os.fspath(explicit) if isinstance(explicit, os.PathLike) else explicit
        root = checked(invoke("experiment.datasets.yaml_root", {
            "explicit": encoded,
            "env": {key: os.environ[key] for key in (module.PROJECTS_ROOT_ENV,) if key in os.environ}}))
        return explicit if root == encoded else root

    def swept_yml_projects(root):
        start = 0
        while True:
            value = checked(invoke("experiment.datasets.yaml_next_project", {
                "root": os.fspath(root), "start": start, "process_cwd": os.getcwd()}))
            if value is None: return
            start = value["next"]
            yield value["name"]

    module.projects_root = projects_root
    module.swept_yml_projects = swept_yml_projects
    module.render = exporter.render
    module.main = lambda: exporter.yaml_main()
