"""Console and filesystem codec for original/native aggregation comparisons."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("aggregate", ROOT / "reference/upstream/experiments/scripts/aggregate.py")
aggregate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(aggregate)
if "--native" in sys.argv:
    from native_aggregate import install
    install(aggregate)

for line in sys.stdin:
    request = json.loads(line)
    stdout, stderr = io.StringIO(), io.StringIO()
    saved = sys.argv
    try:
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            if request["method"] == "main":
                sys.argv = ["aggregate.py", *request["argv"]]
                value = aggregate.main()
            else:
                value = getattr(aggregate, request["method"])(**request.get("params", {}))
                if request["method"] == "summarize":
                    value = [{"key": key, "value": cell} for key, cell in value.items()]
        response = {"value": value}
    except BaseException as error:
        response = {"raised": type(error).__name__, "message": str(error)}
    finally:
        sys.argv = saved
    response.update(stdout=stdout.getvalue(), stderr=stderr.getvalue())
    if "out" in request:
        out = Path(request["out"])
        response["files"] = {name: (out / name).read_bytes().decode("utf-8")
                             for name in ("all_rows.csv", "summary_by_cell.csv") if (out / name).is_file()}
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
