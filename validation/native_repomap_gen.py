"""Exec the native generator through the original tests' Python command interface."""
import os
from pathlib import Path
import sys

binary = Path(__file__).resolve().parents[1] / "target/debug/te-repomap-gen"
os.execv(binary, [str(binary), "--python", sys.executable, *sys.argv[1:]])
