"""Run the pinned recorder's embedded standard-library entry point."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
from te_harness.capture_catalog import RECORDER_SRC
exec(compile(RECORDER_SRC, "<source catalogue recorder>", "exec"))
