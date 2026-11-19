"""Source/native ledger calls for Java-generated mixed numeric rows."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "reference/upstream"))
from tokeneconomizer import ledger
from tokeneconomizer.plugin import accounting
if "--native" in sys.argv:
    from native_plugin import invoke

for line in sys.stdin:
    output = []
    for request in json.loads(line):
        try:
            if "--native" in sys.argv:
                value = invoke(request["op"], request["args"])
            else:
                module = ledger if request["op"].startswith("ledger.") else accounting
                value = getattr(module, request["op"].split(".", 1)[1])(**request["args"])
            output.append(dict(value=value))
        except Exception as error:
            output.append(dict(raised=type(error).__name__, message=str(error)))
    print(json.dumps(output, sort_keys=True, ensure_ascii=True, separators=(",", ":")), flush=True)
