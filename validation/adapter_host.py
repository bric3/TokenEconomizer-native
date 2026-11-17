"""JSON I/O for source/native adapter comparisons; dataset policy stays in the loaded implementation."""
import dataclasses
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
from te_bench.adapters import SwebenchAdapter, WebBenchAdapter, CredDataAdapter, ReposVulAdapter
from te_bench.adapters import swebench

if "--native" in sys.argv:
    from native_plugin import install_experiment_adapters
    install_experiment_adapters()

CLASSES = {"swebench": SwebenchAdapter, "webbench": WebBenchAdapter, "creddata": CredDataAdapter, "reposvul": ReposVulAdapter}
BASE = {key: value for key, value in os.environ.items() if key not in (
    "TOKENECONOMIZER_SWEBENCH_TASKS", "TOKENECONOMIZER_WEBBENCH_PROJECTS", "TOKENECONOMIZER_CREDDATA_ROOT", "TOKENECONOMIZER_REPOSVUL_ROOT")}
BASE_CWD = os.getcwd()

def encode(value):
    if dataclasses.is_dataclass(value):
        return dataclasses.asdict(value)
    if isinstance(value, (set, frozenset)):
        return sorted(value)
    if isinstance(value, os.PathLike):
        return os.fspath(value)
    raise TypeError(type(value).__name__)

for line in sys.stdin:
    request = json.loads(line)
    os.environ.clear()
    os.environ.update(BASE | request.get("env", {}))
    os.chdir(request.get("cwd", BASE_CWD))
    try:
        method = request["method"]
        params = request.get("params", {})
        if method in ("redact_gold_pointers", "untool_exclusions"):
            value = getattr(swebench, method)(**params)
        else:
            arguments = request.get("arguments", {})
            for key in request.get("paths", []):
                arguments[key] = Path(arguments[key])
            for key in request.get("iterators", []):
                arguments[key] = iter(arguments[key])
            adapter = CLASSES[request["domain"]](**arguments)
            os.environ.update(request.get("after_env", {}))
            for key, value in request.get("mutate", {}).items():
                setattr(adapter, key, value)
            if method == "attributes":
                value = {key: getattr(adapter, key) for key in request["attributes"]}
            else:
                value = getattr(adapter, method)(**params)
        response = {"value": value}
    except Exception as error:
        response = {"raised": type(error).__name__}
        if request.get("error_details"):
            response["message"] = str(error)
    print(json.dumps(response, default=encode, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
