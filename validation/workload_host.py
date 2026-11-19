"""Data, object identity and console codecs for workload source comparisons."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
from te_bench import workloads

spec = importlib.util.spec_from_file_location("export_workloads", ROOT / "reference/upstream/experiments/scripts/export_workloads.py")
exporter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exporter)


def constants():
    return {name: getattr(workloads, name) for name in workloads.__all__ if name.isupper()}


if "--dump" in sys.argv:
    print(json.dumps({"constants": constants(), "descriptions": exporter._DOC,
                      "group_fields": exporter._GROUP_FIELD,
                      "notes": exporter.build("swebench")["notes"]},
                     default=sorted, indent=2, ensure_ascii=True))
    raise SystemExit(0)

if "--native" in sys.argv:
    from native_workloads import install
    install(workloads)
    install(exporter)

for line in sys.stdin:
    request = json.loads(line)
    stdout, stderr = io.StringIO(), io.StringIO()
    saved = sys.argv
    try:
        exporter.REPO = request.get("base", str(ROOT / "reference/upstream/experiments"))
        exporter.OUT_DIR = request.get("out", str(Path(exporter.REPO) / "workloads"))
        params = request.get("params", {})
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            if request["method"] == "constants":
                value = constants()
            elif request["method"] == "main":
                sys.argv = ["export_workloads.py", *request.get("argv", [])]
                value = exporter.main()
            elif request["method"] in ("build", "path_for"):
                value = getattr(exporter, request["method"])(**params)
            else:
                value = getattr(workloads, request["method"])(**params)
        response = {"value": value}
        if request["method"] == "filter_tasks":
            response["same_records"] = all(any(item is record for record in params["tasks"]) for item in value)
    except BaseException as error:
        response = {"raised": type(error).__name__, "message": str(error)}
    finally:
        sys.argv = saved
    response.update(stdout=stdout.getvalue(), stderr=stderr.getvalue())
    if request.get("files"):
        out = Path(exporter.OUT_DIR)
        response["files"] = {name: (out / (name + ".json")).read_bytes().decode("utf-8")
                             for name in workloads.DOMAINS if (out / (name + ".json")).is_file()}
    print(json.dumps(response, default=sorted, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
