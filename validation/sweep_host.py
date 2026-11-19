"""Argument and console codecs for original/native sweep comparisons."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("sweep_plan", ROOT / "reference/upstream/experiments/scripts/sweep_plan.py")
sweep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sweep)
if "--native" in sys.argv:
    from native_sweep import install
    install(sweep)

BASE = os.getcwd()
EXPERIMENTS = sweep._EXPERIMENTS
CONSTANTS = ("SWEEP_ARMS", "SWEEP_ARMS_WEBBENCH_OUT_OF_SCOPE", "SWEEP_ARMS_WEBBENCH_UNAVAILABLE",
             "SWEEP_SEEDS", "DEFAULT_SEC_PER_TASK", "DEFAULT_USD_PER_TASK")
for line in sys.stdin:
    request = json.loads(line)
    stdout, stderr = io.StringIO(), io.StringIO()
    saved = sys.argv
    try:
        sweep._EXPERIMENTS = request.get("root", EXPERIMENTS)
        os.chdir(request.get("cwd", BASE))
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            if request["method"] == "constants":
                value = {name: getattr(sweep, name) for name in CONSTANTS}
            elif request["method"] == "main":
                sys.argv = ["sweep_plan.py", *request.get("argv", [])]
                value = sweep.main()
            else:
                value = getattr(sweep, request["method"])(**request.get("params", {}))
        response = {"value": value}
    except BaseException as error:
        response = {"raised": type(error).__name__, "message": str(error)}
    finally:
        sys.argv = saved
        os.chdir(BASE)
    response.update(stdout=stdout.getvalue(), stderr=stderr.getvalue())
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
