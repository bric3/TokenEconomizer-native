"""Original/native Hermes redirect calls for Java-generated JSON fixtures."""
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("redirect_source", ROOT / "reference/upstream/plugin/hermes/__init__.py")
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)
if "--native" in sys.argv:
    from native_plugin import invoke

for line in sys.stdin:
    output = []
    for request in json.loads(line):
        try:
            value = invoke("hermes_gateway.with_redirect", request) if "--native" in sys.argv else source.with_redirect(**request)
            output.append(dict(value=value))
        except Exception as error:
            output.append(dict(raised=type(error).__name__, message=str(error)))
    print(json.dumps(output, sort_keys=True, ensure_ascii=True, separators=(",", ":")), flush=True)
