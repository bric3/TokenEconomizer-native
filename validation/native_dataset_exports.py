"""Bind the native exporter to original module objects and the production SDK adapter."""
import sys

from native_plugin import ROOT
from sdk_transport import ValidationBridge

sys.path.insert(0, str(ROOT / "src/experiments/sdk"))
from te_runtime.datasets import Exporter


def install(module):
    exporter = Exporter(ValidationBridge())
    constants = exporter.constants
    assert module.SPLITS == tuple(constants["splits"])
    assert module.GRADING_FIELDS == tuple(constants["grading_fields"])
    module.SPLITS = tuple(constants["splits"])
    module.GRADING_FIELDS = tuple(constants["grading_fields"])
    module.build = exporter.build
    module.main = lambda: exporter.main()
