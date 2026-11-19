"""Original/native dead-code function and console transport for source comparisons."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("source_dead_code", ROOT / "reference/upstream/scripts/dead_code.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
if "--native" in sys.argv:
    from native_dead_code import install
    install(module)

for line in sys.stdin:
    request = json.loads(line)
    output, errors = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
        try:
            operation = request.get("operation", "scan")
            if operation == "scan": result = module.scan(request["root"])
            elif operation == "fix":
                value = request.get("items")
                if value is None: value = module.scan(request["root"])
                for item in value:
                    for key in ("_span", "_class"):
                        if item.get(key) is not None: item[key] = tuple(item[key])
                result = module.fix(request["root"], value)
            elif operation == "sweep": result = module.sweep(request["root"], request.get("max_passes", 20))
            elif operation == "main":
                original = sys.argv
                try:
                    sys.argv = ["dead_code.py", *request["argv"]]
                    result = module.main(request["argv"])
                finally: sys.argv = original
            else: result = getattr(module, operation)(**request["arguments"])
            response = {"value": result}
        except BaseException as error:
            response = {"raised": type(error).__name__, "message": str(error)}
    response.update(stdout=output.getvalue(), stderr=errors.getvalue())
    print(json.dumps(response, sort_keys=True, ensure_ascii=True, separators=(",", ":")), flush=True)
