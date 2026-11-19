"""Load the unchanged authoring script and redirect its outputs to an owned fixture."""
import importlib.util
from pathlib import Path
import sys

script = Path(__file__).resolve().parents[1] / "reference/upstream/docs/assets" / (sys.argv[1] + ".py")
spec = importlib.util.spec_from_file_location("source_figure", script)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
module.OUT_DIR = Path(sys.argv[2])
module.main()
